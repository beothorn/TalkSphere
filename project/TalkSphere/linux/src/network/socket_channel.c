#include "socket_channel.h"

#include "../common/result.h"
#include "../messageParsing/message_parser.h"
#include "logging.h"

#include <arpa/inet.h>
#include <netinet/in.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#define BUFFER_SIZE 1024
#define RESPONSE_BUFFER_SIZE 16384
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
#define DEFAULT_CONNECT_HOST "127.0.0.1"
#define FETCH_CONNECTED_PEER_OFFERINGS_COMMAND "FETCH_CONNECTED_PEER_OFFERINGS"
#define SEND_CONNECTED_PEER_MESSAGE_PREFIX "SEND_CONNECTED_PEER_MESSAGE:"
#define SUCCESS_RESPONSE_TEXT "OK"
#define LOCAL_CONTROL_MESSAGE_SIZE 2048

static int configure_reusable_socket(
    int socket_file_descriptor
) {
    LOG_TRACE("configure_reusable_socket(): now we allow this socket to rebind after quick restarts");

    int socket_option_enabled = ENABLE_SOCKET_OPTION;
    if (setsockopt(
            socket_file_descriptor,
            SOL_SOCKET,
            SO_REUSEADDR,
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

    memset(local_address, STRUCT_ZERO_FILL, sizeof(*local_address));
    local_address->sin_family = AF_INET;
    local_address->sin_addr.s_addr = htonl(INADDR_ANY);
    local_address->sin_port = htons((uint16_t)local_port);
}

static int build_remote_address(
    struct sockaddr_in *remote_address,
    const char *remote_host,
    int remote_port
) {
    LOG_TRACE("build_remote_address(): now we build an IPv4 address for a remote endpoint");

    memset(remote_address, STRUCT_ZERO_FILL, sizeof(*remote_address));
    remote_address->sin_family = AF_INET;
    remote_address->sin_port = htons((uint16_t)remote_port);

    const char *normalized_host = remote_host;
    if (strcmp(remote_host, "localhost") == 0) {
        normalized_host = DEFAULT_CONNECT_HOST;
    }

    if (inet_pton(AF_INET, normalized_host, &remote_address->sin_addr) <= INVALID_INET_PTON_RESULT) {
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

static int send_all_socket_text(
    int socket_file_descriptor,
    const char *message_text
) {
    LOG_TRACE("send_all_socket_text(): now we write a complete text message to a connected socket");

    size_t message_length = strlen(message_text);
    size_t sent_total_count = 0;
    while (sent_total_count < message_length) {
        ssize_t sent_bytes_count = send(
            socket_file_descriptor,
            message_text + sent_total_count,
            message_length - sent_total_count,
            SEND_FLAGS
        );

        if (sent_bytes_count <= 0) {
            LOG_ERROR("Sending socket text failed so the remote endpoint cannot process the full command");
            return TALKSPHERE_FAILURE;
        }

        sent_total_count += (size_t)sent_bytes_count;
    }

    return TALKSPHERE_SUCCESS;
}

static int connect_to_endpoint(
    const char *remote_host,
    int remote_port,
    int *connected_socket_file_descriptor
) {
    LOG_TRACE("connect_to_endpoint(): now we open a TCP connection to the requested endpoint");

    int client_file_descriptor = create_tcp_socket();
    if (client_file_descriptor == SOCKET_NOT_CREATED_YET) {
        return TALKSPHERE_FAILURE;
    }

    struct sockaddr_in remote_address;
    if (build_remote_address(
            &remote_address,
            remote_host,
            remote_port
        ) != TALKSPHERE_SUCCESS
    ) {
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    if (connect(
            client_file_descriptor,
            (struct sockaddr *)&remote_address,
            sizeof(remote_address)
        ) == SYSTEM_CALL_FAILED
    ) {
        LOG_WARN("Connecting to the endpoint failed so the command cannot reach the requested TalkSphere instance");
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    *connected_socket_file_descriptor = client_file_descriptor;
    return TALKSPHERE_SUCCESS;
}

static int receive_socket_response(
    int socket_file_descriptor,
    char *response_text,
    size_t response_text_size
) {
    LOG_TRACE("receive_socket_response(): now we read a complete response from the connected socket");

    if (response_text == NULL || response_text_size == 0) {
        LOG_WARN("The response buffer is unwanted because a socket response needs writable storage");
        return TALKSPHERE_FAILURE;
    }

    size_t received_total_count = 0;
    while (received_total_count < response_text_size - RECEIVE_BUFFER_TERMINATOR_SPACE) {
        ssize_t received_bytes_count = recv(
            socket_file_descriptor,
            response_text + received_total_count,
            response_text_size - received_total_count - RECEIVE_BUFFER_TERMINATOR_SPACE,
            RECEIVE_FLAGS
        );

        if (received_bytes_count == SYSTEM_CALL_FAILED) {
            LOG_ERROR("Receiving the socket response failed so the command result cannot be trusted");
            return TALKSPHERE_FAILURE;
        }

        if (received_bytes_count == 0) {
            response_text[received_total_count] = STRING_TERMINATOR;
            return TALKSPHERE_SUCCESS;
        }

        received_total_count += (size_t)received_bytes_count;
    }

    response_text[received_total_count] = STRING_TERMINATOR;
    LOG_WARN("The socket response is unwanted because it is larger than the response buffer");
    return TALKSPHERE_FAILURE;
}

static int send_message_to_endpoint(
    const char *remote_host,
    int remote_port,
    const char *message_text
) {
    LOG_TRACE("send_message_to_endpoint(): now we open an outgoing connection and send a single message");

    int client_file_descriptor = SOCKET_NOT_CREATED_YET;
    if (connect_to_endpoint(
            remote_host,
            remote_port,
            &client_file_descriptor
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    int send_result = send_all_socket_text(
        client_file_descriptor,
        message_text
    );
    close(client_file_descriptor);

    return send_result;
}

static int send_message_to_endpoint_with_response(
    const char *remote_host,
    int remote_port,
    const char *message_text,
    char *response_text,
    size_t response_text_size
) {
    LOG_TRACE("send_message_to_endpoint_with_response(): now we send a command and wait for the endpoint response");

    int client_file_descriptor = SOCKET_NOT_CREATED_YET;
    if (connect_to_endpoint(
            remote_host,
            remote_port,
            &client_file_descriptor
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (send_all_socket_text(
            client_file_descriptor,
            message_text
        ) != TALKSPHERE_SUCCESS
    ) {
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    if (shutdown(
            client_file_descriptor,
            SHUT_WR
        ) == SYSTEM_CALL_FAILED
    ) {
        LOG_WARN("Shutting down the socket write side failed so the peer may wait longer before answering");
    }

    int receive_result = receive_socket_response(
        client_file_descriptor,
        response_text,
        response_text_size
    );
    close(client_file_descriptor);

    return receive_result;
}

static int send_response_to_connected_client(
    int connected_client_file_descriptor,
    const char *response_text
) {
    LOG_TRACE("send_response_to_connected_client(): now we return a command response on the accepted socket connection");

    return send_all_socket_text(
        connected_client_file_descriptor,
        response_text
    );
}

int request_remote_offerings_through_client_port(
    int local_client_port,
    char *offerings_text,
    size_t offerings_text_size
) {
    LOG_TRACE("request_remote_offerings_through_client_port(): now the CLI asks the local instance to fetch peer offerings");

    if (send_message_to_endpoint_with_response(
            DEFAULT_CONNECT_HOST,
            local_client_port,
            FETCH_CONNECTED_PEER_OFFERINGS_COMMAND,
            offerings_text,
            offerings_text_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (offerings_text[0] == STRING_TERMINATOR) {
        LOG_WARN("The offerings response is unwanted because the local instance returned no peer offerings");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int build_send_connected_peer_message(
    const char *message_text,
    char *local_control_message_text,
    size_t local_control_message_text_size
) {
    LOG_TRACE("build_send_connected_peer_message(): now we build the local control message that asks the instance to forward user text");

    if (snprintf(
            local_control_message_text,
            local_control_message_text_size,
            "%s%s",
            SEND_CONNECTED_PEER_MESSAGE_PREFIX,
            message_text
        ) >= (int)local_control_message_text_size
    ) {
        LOG_WARN("The message is unwanted because it is too large for the local control socket command");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int request_message_send_through_client_port(
    int local_client_port,
    const char *message_text
) {
    LOG_TRACE("request_message_send_through_client_port(): now the CLI asks the local instance to forward a message to its peer");

    char local_control_message_text[LOCAL_CONTROL_MESSAGE_SIZE];
    if (build_send_connected_peer_message(
            message_text,
            local_control_message_text,
            sizeof(local_control_message_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char response_text[BUFFER_SIZE];
    if (send_message_to_endpoint_with_response(
            DEFAULT_CONNECT_HOST,
            local_client_port,
            local_control_message_text,
            response_text,
            sizeof(response_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (strcmp(
            response_text,
            SUCCESS_RESPONSE_TEXT
        ) != 0
    ) {
        LOG_WARN("The message forwarding response is unwanted because the local instance did not confirm success");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int run_socket_channel(
    int client_port,
    int server_port,
    const char *app_storage_directory_path
) {
    LOG_TRACE("run_socket_channel(): now we run one server instance and process incoming connections forever");

    struct message_processing_dependencies message_processing_dependencies;
    message_processing_dependencies.send_message_to_endpoint = send_message_to_endpoint;
    message_processing_dependencies.send_message_to_endpoint_with_response = send_message_to_endpoint_with_response;
    message_processing_dependencies.listening_port = client_port;
    message_processing_dependencies.peer_port = server_port;
    message_processing_dependencies.app_storage_directory_path = app_storage_directory_path;

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

    if (bind(
            server_file_descriptor,
            (struct sockaddr *)&server_address,
            sizeof(server_address)
        ) == SYSTEM_CALL_FAILED
    ) {
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    if (listen(server_file_descriptor, LISTEN_BACKLOG) == SYSTEM_CALL_FAILED) {
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    printf("Server listening on port %d...\n", client_port);

    while (true) {
        struct sockaddr_in client_address;
        socklen_t client_address_length = sizeof(client_address);
        int connected_client_file_descriptor = accept(
            server_file_descriptor,
            (struct sockaddr *)&client_address,
            &client_address_length
        );

        if (connected_client_file_descriptor == SYSTEM_CALL_FAILED) {
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
            close(connected_client_file_descriptor);
            continue;
        }

        receive_buffer[read_bytes_count] = STRING_TERMINATOR;
        char response_buffer[RESPONSE_BUFFER_SIZE];
        if (process_received_message(
            receive_buffer,
            &message_processing_dependencies,
            response_buffer,
            sizeof(response_buffer)
        ) == TALKSPHERE_SUCCESS && response_buffer[0] != STRING_TERMINATOR
        ) {
            send_response_to_connected_client(
                connected_client_file_descriptor,
                response_buffer
            );
        }
        close(connected_client_file_descriptor);
    }

    return TALKSPHERE_SUCCESS;
}
