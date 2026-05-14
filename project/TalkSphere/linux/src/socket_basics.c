#include "socket_basics.h"

#include "logging.h"

#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/select.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

#define BUFFER_SIZE 1024

#define SOCKET_NOT_CREATED_YET (-1)
#define SYSTEM_CALL_FAILED (-1)
#define STRUCT_ZERO_FILL 0
#define STRING_TERMINATOR '\0'
#define DEFAULT_SOCKET_PROTOCOL 0
#define ENABLE_SOCKET_OPTION 1
#define LISTEN_BACKLOG 1
#define SERVER_WAIT_TIMEOUT_SECONDS 5
#define SERVER_WAIT_TIMEOUT_MICROSECONDS 0
#define NO_READY_FILE_DESCRIPTORS 0
#define SELECT_FILE_DESCRIPTOR_COUNT_OFFSET 1
#define RECEIVE_BUFFER_TERMINATOR_SPACE 1
#define RECEIVE_FLAGS 0
#define SEND_FLAGS 0
#define INVALID_INET_PTON_RESULT 0
#define SERVER_START_DELAY_SECONDS 1

#define DEFAULT_SERVER_IP "127.0.0.1"
#define DEFAULT_MESSAGE "Hello from TalkSphere"

struct server_config {
    int server_port;
};

struct server_result {
    int exit_code;
};

static int configure_reusable_socket(
    int socket_file_descriptor
) {
    TALKSPHERE_LOG_TRACE("configure_reusable_socket(): now we allow this socket to rebind after quick restarts");

    int socket_option_enabled = ENABLE_SOCKET_OPTION;

    if (setsockopt(
            socket_file_descriptor,
            SOL_SOCKET,
            SO_REUSEADDR, // SO_REUSEADDR helps avoid "Address already in use" after quick restarts.
            &socket_option_enabled,
            sizeof(socket_option_enabled)
        ) == SYSTEM_CALL_FAILED
    ) {
        TALKSPHERE_LOG_ERROR("setsockopt failed so this socket cannot be prepared for binding");
        perror("setsockopt");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static void build_local_address(
    struct sockaddr_in *local_address,
    int local_port
) {
    TALKSPHERE_LOG_TRACE("build_local_address(): now we build an IPv4 address that binds on every local interface");
    TALKSPHERE_LOG_DEBUG(
        "Building local socket address for port %d",
        local_port
    );

    memset(
        local_address,
        STRUCT_ZERO_FILL,
        sizeof(*local_address)
    );

    local_address->sin_family = AF_INET; // AF_INET tells the kernel this is an IPv4 address.
    local_address->sin_addr.s_addr = htonl(INADDR_ANY); // INADDR_ANY accepts local traffic on any interface.
    local_address->sin_port = htons((uint16_t)local_port); // sockaddr_in stores ports in network byte order.
}

static int build_remote_address(
    struct sockaddr_in *remote_address,
    const char *server_ip,
    int server_port
) {
    TALKSPHERE_LOG_TRACE("build_remote_address(): now we build the IPv4 address for the server endpoint");
    TALKSPHERE_LOG_DEBUG(
        "Building remote socket address for %s:%d",
        server_ip,
        server_port
    );

    memset(
        remote_address,
        STRUCT_ZERO_FILL,
        sizeof(*remote_address)
    );

    remote_address->sin_family = AF_INET; // AF_INET tells connect that this is an IPv4 server.
    remote_address->sin_port = htons((uint16_t)server_port); // TCP expects network byte order here.

    if (inet_pton(
            AF_INET,
            server_ip,
            &remote_address->sin_addr
        ) <= INVALID_INET_PTON_RESULT
    ) {
        TALKSPHERE_LOG_ERROR("The server IP is unwanted because it cannot be converted into an IPv4 address");
        fprintf(
            stderr,
            "Invalid server IP: %s\n",
            server_ip
        );
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int create_tcp_socket(void) {
    TALKSPHERE_LOG_TRACE("create_tcp_socket(): now we ask the kernel for an IPv4 TCP socket");

    int socket_file_descriptor = socket(
        AF_INET, // AF_INET = IPv4.
        SOCK_STREAM, // SOCK_STREAM = TCP stream socket.
        DEFAULT_SOCKET_PROTOCOL
    );

    if (socket_file_descriptor == SYSTEM_CALL_FAILED) {
        TALKSPHERE_LOG_ERROR("socket failed so no network endpoint can be created");
        perror("socket");
        return SOCKET_NOT_CREATED_YET;
    }

    return socket_file_descriptor;
}

static int wait_for_client_connection(
    int server_file_descriptor
) {
    TALKSPHERE_LOG_TRACE("wait_for_client_connection(): now we wait briefly until the server socket is readable");

    fd_set read_file_descriptors;
    FD_ZERO(&read_file_descriptors); // Clear the set before adding file descriptors.
    FD_SET(
        server_file_descriptor,
        &read_file_descriptors
    ); // Monitor the server socket for incoming connections.

    struct timeval timeout;
    timeout.tv_sec = SERVER_WAIT_TIMEOUT_SECONDS;
    timeout.tv_usec = SERVER_WAIT_TIMEOUT_MICROSECONDS;

    int ready_file_descriptor_count = select(
        server_file_descriptor + SELECT_FILE_DESCRIPTOR_COUNT_OFFSET,
        &read_file_descriptors,
        NULL,
        NULL,
        &timeout
    );

    if (ready_file_descriptor_count == SYSTEM_CALL_FAILED) {
        TALKSPHERE_LOG_ERROR("select failed so the server cannot know whether a client is waiting");
        perror("select");
        return TALKSPHERE_FAILURE;
    }

    if (ready_file_descriptor_count == NO_READY_FILE_DESCRIPTORS) {
        TALKSPHERE_LOG_WARN("Waiting timed out because no client connected before the demo timeout");
        fprintf(
            stderr,
            "Timed out waiting for a client connection.\n"
        );
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int receive_client_message(
    int connected_client_file_descriptor,
    const struct sockaddr_in *client_address
) {
    TALKSPHERE_LOG_TRACE("receive_client_message(): now we receive bytes and print them as a message");

    char receive_buffer[BUFFER_SIZE];

    ssize_t bytes_read = recv(
        connected_client_file_descriptor,
        receive_buffer,
        sizeof(receive_buffer) - RECEIVE_BUFFER_TERMINATOR_SPACE,
        RECEIVE_FLAGS
    );

    if (bytes_read == SYSTEM_CALL_FAILED) {
        TALKSPHERE_LOG_ERROR("recv failed so the server could not read the client message");
        perror("recv");
        return TALKSPHERE_FAILURE;
    }

    TALKSPHERE_LOG_DEBUG(
        "Received %zd bytes from client",
        bytes_read
    );

    receive_buffer[bytes_read] = STRING_TERMINATOR; // Make received bytes printable as a C string.

    char client_ip[INET_ADDRSTRLEN] = {STRUCT_ZERO_FILL};
    inet_ntop(
        AF_INET,
        &client_address->sin_addr,
        client_ip,
        sizeof(client_ip)
    );

    printf(
        "Received from %s:%d -> %s\n",
        client_ip,
        ntohs(client_address->sin_port),
        receive_buffer
    );

    return TALKSPHERE_SUCCESS;
}

static int run_server(
    int server_port
) {
    TALKSPHERE_LOG_TRACE("run_server(): now the server opens a socket, waits for one client, and reads one message");

    int server_file_descriptor = create_tcp_socket();

    if (server_file_descriptor == SOCKET_NOT_CREATED_YET) {
        return TALKSPHERE_FAILURE;
    }

    if (configure_reusable_socket(server_file_descriptor) != TALKSPHERE_SUCCESS) {
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    struct sockaddr_in server_address;
    build_local_address(
        &server_address,
        server_port
    );

    if (bind(
            server_file_descriptor,
            (struct sockaddr *)&server_address,
            sizeof(server_address)
        ) == SYSTEM_CALL_FAILED
    ) {
        TALKSPHERE_LOG_ERROR("bind failed so the server cannot listen on the requested port");
        perror("bind");
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    if (listen(
            server_file_descriptor,
            LISTEN_BACKLOG
        ) == SYSTEM_CALL_FAILED
    ) {
        TALKSPHERE_LOG_ERROR("listen failed so the server cannot accept client connections");
        perror("listen");
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    TALKSPHERE_LOG_INFO("Server is listening successfully");
    printf(
        "Server listening on port %d...\n",
        server_port
    );

    if (wait_for_client_connection(server_file_descriptor) != TALKSPHERE_SUCCESS) {
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    struct sockaddr_in client_address;
    socklen_t client_address_length = sizeof(client_address);

    int connected_client_file_descriptor = accept(
        server_file_descriptor,
        (struct sockaddr *)&client_address,
        &client_address_length
    );

    if (connected_client_file_descriptor == SYSTEM_CALL_FAILED) {
        TALKSPHERE_LOG_ERROR("accept failed so the server could not receive the waiting client");
        perror("accept");
        close(server_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    int receive_status = receive_client_message(
        connected_client_file_descriptor,
        &client_address
    );

    close(connected_client_file_descriptor);
    close(server_file_descriptor);
    return receive_status;
}

static void *server_thread_main(
    void *raw_config
) {
    TALKSPHERE_LOG_TRACE("server_thread_main(): now we run the server on a background thread");

    struct server_config *server_config = (struct server_config *)raw_config;
    struct server_result *server_result = malloc(sizeof(*server_result));

    if (server_result == NULL) {
        TALKSPHERE_LOG_ERROR("malloc failed so the server thread cannot return a structured result");
        perror("malloc");
        return NULL;
    }

    server_result->exit_code = run_server(server_config->server_port);
    return server_result;
}

static int run_client(
    const char *server_ip,
    const char *message,
    int client_port,
    int server_port
) {
    TALKSPHERE_LOG_TRACE("run_client(): now the client binds locally, connects to the server, and sends one message");

    int client_file_descriptor = create_tcp_socket();

    if (client_file_descriptor == SOCKET_NOT_CREATED_YET) {
        return TALKSPHERE_FAILURE;
    }

    if (configure_reusable_socket(client_file_descriptor) != TALKSPHERE_SUCCESS) {
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    struct sockaddr_in local_address;
    build_local_address(
        &local_address,
        client_port
    );

    if (bind(
            client_file_descriptor,
            (struct sockaddr *)&local_address,
            sizeof(local_address)
        ) == SYSTEM_CALL_FAILED
    ) {
        TALKSPHERE_LOG_ERROR("bind failed so the client cannot use the requested source port");
        perror("bind");
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    struct sockaddr_in server_address;

    if (build_remote_address(
            &server_address,
            server_ip,
            server_port
        ) != TALKSPHERE_SUCCESS
    ) {
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    if (connect(
            client_file_descriptor,
            (struct sockaddr *)&server_address,
            sizeof(server_address)
        ) == SYSTEM_CALL_FAILED
    ) {
        TALKSPHERE_LOG_ERROR("connect failed so the client could not reach the local demo server");
        perror("connect");
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    size_t message_length = strlen(message);
    ssize_t bytes_sent = send(
        client_file_descriptor,
        message,
        message_length,
        SEND_FLAGS
    );

    if (bytes_sent != (ssize_t)message_length) {
        TALKSPHERE_LOG_ERROR("send failed so the demo message may not have reached the server");
        perror("send");
        close(client_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    TALKSPHERE_LOG_INFO("Client sent the demo message successfully");
    printf(
        "Sent message to %s:%d from local port %d\n",
        server_ip,
        server_port,
        client_port
    );

    close(client_file_descriptor);
    return TALKSPHERE_SUCCESS;
}

int run_socket_basics(
    const struct program_arguments *program_arguments
) {
    TALKSPHERE_LOG_TRACE("run_socket_basics(): now we coordinate the server thread and the client run");

    struct server_config server_config = {
        .server_port = program_arguments->server_port
    };
    pthread_t server_thread;

    int thread_status = pthread_create(
        &server_thread,
        NULL,
        server_thread_main,
        &server_config
    );

    if (thread_status != TALKSPHERE_SUCCESS) {
        TALKSPHERE_LOG_ERROR("pthread_create failed so the server cannot run beside the client");
        errno = thread_status;
        perror("pthread_create");
        return TALKSPHERE_FAILURE;
    }

    sleep(SERVER_START_DELAY_SECONDS);

    int client_status = run_client(
        DEFAULT_SERVER_IP,
        DEFAULT_MESSAGE,
        program_arguments->client_port,
        program_arguments->server_port
    );

    void *raw_thread_result = NULL;
    thread_status = pthread_join(
        server_thread,
        &raw_thread_result
    );

    if (thread_status != TALKSPHERE_SUCCESS) {
        TALKSPHERE_LOG_ERROR("pthread_join failed so the server result cannot be collected");
        errno = thread_status;
        perror("pthread_join");
        return TALKSPHERE_FAILURE;
    }

    int server_status = TALKSPHERE_FAILURE;

    if (raw_thread_result != NULL) {
        struct server_result *server_result = (struct server_result *)raw_thread_result;
        server_status = server_result->exit_code;
        free(server_result);
    }

    if (client_status == TALKSPHERE_SUCCESS && server_status == TALKSPHERE_SUCCESS) {
        TALKSPHERE_LOG_INFO("Socket basics demo finished successfully");
    }

    return client_status != TALKSPHERE_SUCCESS ? client_status : server_status;
}
