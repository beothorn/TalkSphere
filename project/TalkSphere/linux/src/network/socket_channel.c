#include "socket_channel.h"

#include "../common/result.h"
#include "logging.h"

#include <arpa/inet.h>
#include <netinet/in.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#define BUFFER_SIZE 1024

#define SOCKET_NOT_CREATED_YET (-1)
#define SYSTEM_CALL_FAILED (-1)
#define STRUCT_ZERO_FILL 0
#define STRING_TERMINATOR '\0'
#define DEFAULT_SOCKET_PROTOCOL 0
#define ENABLE_SOCKET_OPTION 1
#define LISTEN_BACKLOG 8
#define RECEIVE_BUFFER_TERMINATOR_SPACE 1
#define RECEIVE_FLAGS 0
#define SEND_FLAGS 0
#define INVALID_INET_PTON_RESULT 0

#define CONNECT_PREFIX "CONNECT:"
#define FROM_PREFIX ",FROM:"
#define MESSAGE_PREFIX "MESSAGE:"
#define DEFAULT_CONNECT_HOST "127.0.0.1"

struct connect_instruction {
    char target_host[INET_ADDRSTRLEN];
    int target_port;
    char reply_host[INET_ADDRSTRLEN];
    int reply_port;
};

static int configure_reusable_socket(
    int socket_file_descriptor
) {
    LOG_TRACE("configure_reusable_socket(): now we allow this socket to rebind after quick restarts");

    int socket_option_enabled = ENABLE_SOCKET_OPTION;

    if (setsockopt(
            socket_file_descriptor,
            SOL_SOCKET,
            SO_REUSEADDR, // SO_REUSEADDR helps avoid "Address already in use" after quick restarts.
            &socket_option_enabled,
            sizeof(socket_option_enabled)
        ) == SYSTEM_CALL_FAILED
    ) {
        LOG_ERROR("setsockopt failed so this socket cannot be prepared for binding");
        perror("setsockopt");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static void build_local_address(
    struct sockaddr_in *local_address,
    int local_port
) {
    LOG_TRACE("build_local_address(): now we build an IPv4 address that binds on every local interface");
    LOG_DEBUG("Building local socket address for port %d", local_port);

    memset(local_address, STRUCT_ZERO_FILL, sizeof(*local_address));
    local_address->sin_family = AF_INET; // AF_INET tells the kernel this is an IPv4 address.
    local_address->sin_addr.s_addr = htonl(INADDR_ANY); // INADDR_ANY accepts local traffic on any interface.
    local_address->sin_port = htons((uint16_t)local_port); // sockaddr_in stores ports in network byte order.
}

static int build_remote_address(
    struct sockaddr_in *remote_address,
    const char *remote_host,
    int remote_port
) {
    LOG_TRACE("build_remote_address(): now we build an IPv4 address for a remote endpoint");
    LOG_DEBUG("Building remote address for %s:%d", remote_host, remote_port);

    memset(remote_address, STRUCT_ZERO_FILL, sizeof(*remote_address));
    remote_address->sin_family = AF_INET; // AF_INET tells connect that this is an IPv4 endpoint.
    remote_address->sin_port = htons((uint16_t)remote_port); // TCP expects network byte order.

    if (strcmp(remote_host, "localhost") == 0) {
        remote_host = DEFAULT_CONNECT_HOST;
    }

    if (inet_pton(AF_INET, remote_host, &remote_address->sin_addr) <= INVALID_INET_PTON_RESULT) {
        LOG_WARN("The remote host is unwanted because it cannot be converted into an IPv4 address");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int create_tcp_socket(void) {
    LOG_TRACE("create_tcp_socket(): now we ask the kernel for an IPv4 TCP socket");

    int socket_file_descriptor = socket(AF_INET, SOCK_STREAM, DEFAULT_SOCKET_PROTOCOL);

    if (socket_file_descriptor == SYSTEM_CALL_FAILED) {
        LOG_ERROR("socket failed so no network endpoint can be created");
        perror("socket");
        return SOCKET_NOT_CREATED_YET;
    }

    return socket_file_descriptor;
}

static int parse_connect_instruction(
    const char *message_text,
    struct connect_instruction *connect_instruction
) {
    LOG_TRACE("parse_connect_instruction(): now we parse a connect request coming from an external caller");
    LOG_DEBUG("Parsing instruction text: %s", message_text);

    int scanned_fields = sscanf(
        message_text,
        "CONNECT:%15[^:]:%d,FROM:%15[^:]:%d",
        connect_instruction->target_host,
        &connect_instruction->target_port,
        connect_instruction->reply_host,
        &connect_instruction->reply_port
    );

    if (scanned_fields != 4) {
        LOG_WARN("The input message is unwanted because it does not match the CONNECT message shape");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int send_message_to_endpoint(
    const char *remote_host,
    int remote_port,
    const char *message_text
) {
    LOG_TRACE("send_message_to_endpoint(): now we open an outgoing connection and send a single message");
    LOG_DEBUG("Sending '%s' to %s:%d", message_text, remote_host, remote_port);

    int client_file_descriptor = create_tcp_socket();
    if (client_file_descriptor == SOCKET_NOT_CREATED_YET) {
        return TALKSPHERE_FAILURE;
    }

    struct sockaddr_in remote_address;
    if (build_remote_address(&remote_address, remote_host, remote_port) != TALKSPHERE_SUCCESS) {
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    if (connect(client_file_descriptor, (struct sockaddr *)&remote_address, sizeof(remote_address)) == SYSTEM_CALL_FAILED) {
        LOG_ERROR("connect failed so this peer could not reach the requested remote endpoint");
        perror("connect");
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    size_t message_length = strlen(message_text);
    ssize_t sent_bytes_count = send(client_file_descriptor, message_text, message_length, SEND_FLAGS);
    if (sent_bytes_count != (ssize_t)message_length) {
        LOG_ERROR("send failed so the remote endpoint did not receive the full message");
        perror("send");
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    close(client_file_descriptor);
    return TALKSPHERE_SUCCESS;
}

static int handle_connect_instruction(
    const struct connect_instruction *connect_instruction,
    int listening_port
) {
    LOG_TRACE("handle_connect_instruction(): now this server connects to the target and sends a hello to the caller endpoint");

    if (connect_instruction->target_port == listening_port) {
        LOG_WARN("The target port is unwanted because it would cause a self-loop connection");
        return TALKSPHERE_FAILURE;
    }

    if (send_message_to_endpoint(connect_instruction->target_host, connect_instruction->target_port, "MESSAGE:Hello") != TALKSPHERE_SUCCESS) {
        LOG_WARN("The target peer could not be reached so the chain request cannot complete");
        return TALKSPHERE_FAILURE;
    }

    if (send_message_to_endpoint(connect_instruction->reply_host, connect_instruction->reply_port, "MESSAGE:Hello") != TALKSPHERE_SUCCESS) {
        LOG_WARN("The reply endpoint could not be reached so the caller did not receive the hello message");
        return TALKSPHERE_FAILURE;
    }

    LOG_INFO("A CONNECT request completed and hello was sent");
    return TALKSPHERE_SUCCESS;
}

static int process_received_message(
    const char *message_text,
    int listening_port
) {
    LOG_TRACE("process_received_message(): now we branch based on the incoming message type");

    if (strncmp(message_text, CONNECT_PREFIX, strlen(CONNECT_PREFIX)) == 0) {
        struct connect_instruction connect_instruction;
        if (parse_connect_instruction(message_text, &connect_instruction) != TALKSPHERE_SUCCESS) {
            return TALKSPHERE_FAILURE;
        }
        return handle_connect_instruction(&connect_instruction, listening_port);
    }

    if (strncmp(message_text, MESSAGE_PREFIX, strlen(MESSAGE_PREFIX)) == 0) {
        const char *message_payload = message_text + strlen(MESSAGE_PREFIX);
        printf("%s\n", message_payload);
        fflush(stdout);
        LOG_INFO("A peer message was printed to stdout");
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("The message type is unwanted because this server only knows CONNECT and MESSAGE");
    return TALKSPHERE_FAILURE;
}

int run_socket_channel(
    int client_port,
    int server_port
) {
    LOG_TRACE("run_socket_channel(): now we run one server instance and process incoming connections forever");
    LOG_DEBUG("run_socket_channel(): client_port=%d, server_port=%d", client_port, server_port);

    (void)server_port;

    int server_file_descriptor = create_tcp_socket();
    if (server_file_descriptor == SOCKET_NOT_CREATED_YET) {
        return TALKSPHERE_FAILURE;
    }

    if (configure_reusable_socket(server_file_descriptor) != TALKSPHERE_SUCCESS) {
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    struct sockaddr_in server_address;
    build_local_address(&server_address, client_port);

    if (bind(server_file_descriptor, (struct sockaddr *)&server_address, sizeof(server_address)) == SYSTEM_CALL_FAILED) {
        LOG_ERROR("bind failed so the server cannot listen on the requested port");
        perror("bind");
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    if (listen(server_file_descriptor, LISTEN_BACKLOG) == SYSTEM_CALL_FAILED) {
        LOG_ERROR("listen failed so the server cannot accept client connections");
        perror("listen");
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    LOG_INFO("Server is listening for peer messages");
    printf("Server listening on port %d...\n", client_port);

    while (true) {
        LOG_TRACE("run_socket_channel(): now the loop waits for the next connection");

        struct sockaddr_in client_address;
        socklen_t client_address_length = sizeof(client_address);
        int connected_client_file_descriptor = accept(
            server_file_descriptor,
            (struct sockaddr *)&client_address,
            &client_address_length
        );

        if (connected_client_file_descriptor == SYSTEM_CALL_FAILED) {
            LOG_ERROR("accept failed so the server could not receive the waiting client");
            perror("accept");
            continue;
        }

        char receive_buffer[BUFFER_SIZE];
        ssize_t read_bytes_count = recv(
            connected_client_file_descriptor,
            receive_buffer,
            sizeof(receive_buffer) - RECEIVE_BUFFER_TERMINATOR_SPACE,
            RECEIVE_FLAGS
        );

        if (read_bytes_count == SYSTEM_CALL_FAILED) {
            LOG_WARN("recv failed so this specific connection could not be parsed");
            perror("recv");
            close(connected_client_file_descriptor);
            continue;
        }

        receive_buffer[read_bytes_count] = STRING_TERMINATOR;
        LOG_DEBUG("Received message text: %s", receive_buffer);

        process_received_message(receive_buffer, client_port);
        close(connected_client_file_descriptor);
    }

    return TALKSPHERE_SUCCESS;
}
