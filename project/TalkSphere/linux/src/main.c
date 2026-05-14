/*
 * Entry point for TalkSphere.
 */
#include <arpa/inet.h>   // inet_pton, inet_ntop
#include <errno.h>       // errno values set by failed syscalls
#include <netinet/in.h>  // sockaddr_in, htons, htonl
#include <pthread.h>     // pthread_create, pthread_join
#include <stdio.h>       // printf, fprintf, perror
#include <stdlib.h>      // general utilities
#include <string.h>      // memset, strlen
#include <sys/select.h>  // select, FD_SET
#include <sys/socket.h>  // socket APIs: socket, bind, listen, accept, connect, send, recv
#include <sys/types.h>   // basic system data types
#include <unistd.h>      // close

// Port where the server waits for incoming TCP connections.
#define DEFAULT_SERVER_PORT 8513
// Port the client binds to locally before connecting.
#define DEFAULT_CLIENT_PORT 8512
// Max number of bytes we receive in one recv() call.
#define BUFFER_SIZE 1024

#define SUCCESS 0
#define FAILURE 1

#define PROGRAM_NAME_ARG_INDEX 0
#define FIRST_CHAR_INDEX 0
#define CLIENT_PORT_ARG_INDEX 1
#define SERVER_PORT_ARG_INDEX 2
#define DEFAULT_ARG_COUNT 1
#define CUSTOM_PORT_ARG_COUNT 3

#define DECIMAL_BASE 10
#define MIN_PORT 1
#define MAX_PORT 65535

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
#define SELECT_NFDS_OFFSET 1
#define RECEIVE_BUFFER_TERMINATOR_SPACE 1
#define RECV_FLAGS 0
#define SEND_FLAGS 0
#define INVALID_INET_PTON_RESULT 0
#define SERVER_START_DELAY_SECONDS 1

#define DEFAULT_SERVER_IP "127.0.0.1"

struct server_config {
    int port;
};

struct server_result {
    int exit_code;
};

/*
 * Print usage instructions.
 */
static void usage(const char *program_name) {
    fprintf(stderr,
            "Usage:\n"
            "  %s\n"
            "  %s <client_port> <server_port>\n\n"
            "Ports:\n"
            "  Default client port: %d\n"
            "  Default server port: %d\n",
            program_name, 
            program_name, 
            DEFAULT_CLIENT_PORT, 
            DEFAULT_SERVER_PORT
        );
}

static int parse_port(
    const char *text, 
    const char *name, 
    int *port
) {
    char *end = NULL;
    long value = strtol(text, &end, DECIMAL_BASE);

    if (text[FIRST_CHAR_INDEX] == STRING_TERMINATOR
        || *end != STRING_TERMINATOR
        || value < MIN_PORT
        || value > MAX_PORT) {
        fprintf(stderr, "Invalid %s port: %s\n", name, text);
        return FAILURE;
    }

    *port = (int)value;
    return SUCCESS;
}

/*
 * Run server mode:
 * 1) Create TCP socket.
 * 2) Allow fast rebinding after restart (SO_REUSEADDR).
 * 3) Bind to 0.0.0.0:8513.
 * 4) Listen for one queued connection.
 * 5) Accept one client.
 * 6) Receive and print message.
 */
static int run_server(
    int server_port
) {
    int server_file_descriptor = SOCKET_NOT_CREATED_YET;
    int connected_client_file_descriptor = SOCKET_NOT_CREATED_YET;

    struct sockaddr_in server_address;
    struct sockaddr_in client_address;

    socklen_t client_len = sizeof(client_address);

    // Receive buffer (extra byte will be reserved for '\0').
    char buffer[BUFFER_SIZE];

    // AF_INET = IPv4, SOCK_STREAM = TCP stream socket.
    server_file_descriptor = socket(AF_INET, SOCK_STREAM, DEFAULT_SOCKET_PROTOCOL);
    if (server_file_descriptor == SYSTEM_CALL_FAILED) {
        perror("socket");
        return FAILURE;
    }

    // SO_REUSEADDR helps avoid "Address already in use" after quick restarts.
    int opt = ENABLE_SOCKET_OPTION;
    if (setsockopt(server_file_descriptor, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) == SYSTEM_CALL_FAILED) {
        perror("setsockopt");
        close(server_file_descriptor);
        return FAILURE;
    }

    // Always zero structs before filling selected fields.
    memset(&server_address, STRUCT_ZERO_FILL, sizeof(server_address));
    server_address.sin_family = AF_INET;
    // INADDR_ANY means “accept connections on all local interfaces”.
    server_address.sin_addr.s_addr = htonl(INADDR_ANY);
    // Ports in sockaddr_in must be in network byte order.
    server_address.sin_port = htons((uint16_t)server_port);

    // Associate socket fd with local ip:port.
    if (bind(server_file_descriptor, (struct sockaddr *)&server_address, sizeof(server_address)) == SYSTEM_CALL_FAILED) {
        perror("bind");
        close(server_file_descriptor);
        return FAILURE;
    }

    // Start passive listening.
    if (listen(server_file_descriptor, LISTEN_BACKLOG) == SYSTEM_CALL_FAILED) {
        perror("listen");
        close(server_file_descriptor);
        return FAILURE;
    }

    printf("Server listening on port %d...\n", server_port);

    fd_set read_fds;
    FD_ZERO(&read_fds);
    FD_SET(server_file_descriptor, &read_fds);

    struct timeval timeout;
    timeout.tv_sec = SERVER_WAIT_TIMEOUT_SECONDS;
    timeout.tv_usec = SERVER_WAIT_TIMEOUT_MICROSECONDS;

    int ready = select(server_file_descriptor + SELECT_NFDS_OFFSET, &read_fds, NULL, NULL, &timeout);
    if (ready == SYSTEM_CALL_FAILED) {
        perror("select");
        close(server_file_descriptor);
        return FAILURE;
    }
    if (ready == NO_READY_FILE_DESCRIPTORS) {
        fprintf(stderr, "Timed out waiting for a client connection.\n");
        close(server_file_descriptor);
        return FAILURE;
    }

    // Accept the client that select reported as ready.
    connected_client_file_descriptor = accept(server_file_descriptor, (struct sockaddr *)&client_address, &client_len);
    if (connected_client_file_descriptor == SYSTEM_CALL_FAILED) {
        perror("accept");
        close(server_file_descriptor);
        return FAILURE;
    }

    // Read bytes from client. recv returns byte count, 0, or SYSTEM_CALL_FAILED on error.
    ssize_t bytes_read = recv(connected_client_file_descriptor, buffer, sizeof(buffer) - RECEIVE_BUFFER_TERMINATOR_SPACE, RECV_FLAGS);
    if (bytes_read == SYSTEM_CALL_FAILED) {
        perror("recv");
        close(connected_client_file_descriptor);
        close(server_file_descriptor);
        return FAILURE;
    }

    // Make received bytes printable as a C string.
    buffer[bytes_read] = STRING_TERMINATOR;

    // Convert client address from binary to printable dotted-decimal text.
    char client_ip[INET_ADDRSTRLEN] = {STRUCT_ZERO_FILL};
    inet_ntop(AF_INET, &client_address.sin_addr, client_ip, sizeof(client_ip));

    printf("Received from %s:%d -> %s\n",
           client_ip,
           ntohs(client_address.sin_port), // client source port (host byte order)
           buffer);

    // Cleanup descriptors when done.
    close(connected_client_file_descriptor);
    close(server_file_descriptor);
    return SUCCESS;
}

static void *server_thread_main(
    void *arg
) {
    struct server_config *config = (struct server_config *)arg;
    struct server_result *result = malloc(sizeof(*result));

    if (result == NULL) {
        perror("malloc");
        return NULL;
    }

    result->exit_code = run_server(config->port);
    return result;
}

/*
 * Run client mode:
 * 1) Create TCP socket.
 * 2) Bind client side to fixed local port 8512.
 * 3) Parse destination IP.
 * 4) Connect to server on port 8513.
 * 5) Send message bytes.
 */
static int run_client(
    const char *server_ip, 
    const char *message, 
    int client_port, 
    int server_port
) {
    int sock_fd = SOCKET_NOT_CREATED_YET;
    struct sockaddr_in local_addr;
    struct sockaddr_in server_address;

    sock_fd = socket(AF_INET, SOCK_STREAM, DEFAULT_SOCKET_PROTOCOL);
    if (sock_fd == SYSTEM_CALL_FAILED) {
        perror("socket");
        return FAILURE;
    }

    int opt = ENABLE_SOCKET_OPTION;
    if (setsockopt(sock_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) == SYSTEM_CALL_FAILED) {
        perror("setsockopt");
        close(sock_fd);
        return FAILURE;
    }

    // Local side endpoint for this client socket.
    memset(&local_addr, STRUCT_ZERO_FILL, sizeof(local_addr));
    local_addr.sin_family = AF_INET;
    local_addr.sin_addr.s_addr = htonl(INADDR_ANY);
    local_addr.sin_port = htons((uint16_t)client_port);

    // Explicit bind makes source port deterministic for learning/demo.
    if (bind(sock_fd, (struct sockaddr *)&local_addr, sizeof(local_addr)) == SYSTEM_CALL_FAILED) {
        perror("bind");
        close(sock_fd);
        return FAILURE;
    }

    // Remote server endpoint.
    memset(&server_address, STRUCT_ZERO_FILL, sizeof(server_address));
    server_address.sin_family = AF_INET;
    server_address.sin_port = htons((uint16_t)server_port);

    // Convert input string like "127.0.0.1" to binary network address.
    if (inet_pton(AF_INET, server_ip, &server_address.sin_addr) <= INVALID_INET_PTON_RESULT) {
        fprintf(stderr, "Invalid server IP: %s\n", server_ip);
        close(sock_fd);
        return FAILURE;
    }

    // Perform TCP 3-way handshake with the server.
    if (connect(sock_fd, (struct sockaddr *)&server_address, sizeof(server_address)) == SYSTEM_CALL_FAILED) {
        perror("connect");
        close(sock_fd);
        return FAILURE;
    }

    // Send the message exactly as provided.
    size_t msg_len = strlen(message);
    if (send(sock_fd, message, msg_len, SEND_FLAGS) != (ssize_t)msg_len) {
        perror("send");
        close(sock_fd);
        return FAILURE;
    }

    printf("Sent message to %s:%d from local port %d\n", server_ip, server_port, client_port);

    close(sock_fd);
    return SUCCESS;
}

/*
 * Entry point:
 * - No arguments uses default client/server ports.
 * - Two arguments override the client bind port and server listen port.
 */
int main(
    int argc, 
    char *argv[]
) {
    int client_port = DEFAULT_CLIENT_PORT;
    int server_port = DEFAULT_SERVER_PORT;

    const char *program_name = argv[PROGRAM_NAME_ARG_INDEX];

    if (argc != DEFAULT_ARG_COUNT && argc != CUSTOM_PORT_ARG_COUNT) {
        // * argv[PROGRAM_NAME_ARG_INDEX] usually contains executable name, so we reuse it in help text.
        usage(program_name);
        return FAILURE;
    }

    if (argc == CUSTOM_PORT_ARG_COUNT) {
        if (parse_port(argv[CLIENT_PORT_ARG_INDEX], "client", &client_port) != SUCCESS ||
            parse_port(argv[SERVER_PORT_ARG_INDEX], "server", &server_port) != SUCCESS) {
            usage(program_name);
            return FAILURE;
        }
    }

    if (client_port == server_port) {
        fprintf(stderr, "Client and server ports must be different.\n");
        return FAILURE;
    }

    struct server_config config = {.port = server_port};
    pthread_t server_thread;

    int thread_status = pthread_create(&server_thread, NULL, server_thread_main, &config);
    if (thread_status != SUCCESS) {
        errno = thread_status;
        perror("pthread_create");
        return FAILURE;
    }

    sleep(SERVER_START_DELAY_SECONDS);

    int client_status = run_client(
        DEFAULT_SERVER_IP, 
        "Hello from TalkSphere", 
        client_port, 
        server_port
    );

    void *thread_result = NULL;
    thread_status = pthread_join(server_thread, &thread_result);
    if (thread_status != SUCCESS) {
        errno = thread_status;
        perror("pthread_join");
        return FAILURE;
    }

    int server_status = FAILURE;
    if (thread_result != NULL) {
        struct server_result *result = (struct server_result *)thread_result;
        server_status = result->exit_code;
        free(result);
    }

    return client_status != SUCCESS ? client_status : server_status;
}
