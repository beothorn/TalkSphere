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

static int send_message_to_endpoint(
    const char *remote_host,
    int remote_port,
    const char *message_text
) {
    LOG_TRACE("send_message_to_endpoint(): now we open an outgoing connection and send a single message");

    int client_file_descriptor = create_tcp_socket();
    if (client_file_descriptor == SOCKET_NOT_CREATED_YET) {
        return TALKSPHERE_FAILURE;
    }

    struct sockaddr_in remote_address;
    if (build_remote_address(&remote_address, remote_host, remote_port) != TALKSPHERE_SUCCESS) {
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    if (connect(
            client_file_descriptor,
            (struct sockaddr *)&remote_address,
            sizeof(remote_address)
        ) == SYSTEM_CALL_FAILED
    ) {
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    size_t message_length = strlen(message_text);
    ssize_t sent_bytes_count = send(
        client_file_descriptor,
        message_text,
        message_length,
        SEND_FLAGS
    );

    close(client_file_descriptor);

    return sent_bytes_count == (ssize_t)message_length
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

int run_socket_channel(
    int client_port,
    int server_port,
    const char *app_storage_directory_path
) {
    LOG_TRACE("run_socket_channel(): now we run one server instance and process incoming connections forever");

    (void)server_port;

    struct message_processing_dependencies message_processing_dependencies;
    message_processing_dependencies.send_message_to_endpoint = send_message_to_endpoint;
    message_processing_dependencies.listening_port = client_port;
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
        process_received_message(
            receive_buffer,
            &message_processing_dependencies
        );
        close(connected_client_file_descriptor);
    }

    return TALKSPHERE_SUCCESS;
}
