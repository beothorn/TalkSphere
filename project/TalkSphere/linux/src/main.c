/*
 * Simple TCP client/server teaching example.
 *
 * Why this file exists:
 * - Demonstrate classic BSD sockets in C.
 * - Keep client and server behavior in one binary for easy learning.
 * - Use fixed ports so you can clearly observe source/destination ports:
 *     server listens on 8513, client binds to 8512.
 */

#include <arpa/inet.h>   // inet_pton, inet_ntop
#include <errno.h>       // errno values set by failed syscalls
#include <netinet/in.h>  // sockaddr_in, htons, htonl
#include <stdio.h>       // printf, fprintf, perror
#include <stdlib.h>      // general utilities
#include <string.h>      // memset, strcmp, strlen
#include <sys/socket.h>  // socket APIs: socket, bind, listen, accept, connect, send, recv
#include <sys/types.h>   // basic system data types
#include <unistd.h>      // close

// Port where the server waits for incoming TCP connections.
#define SERVER_PORT 8513
// Port the client binds to locally before connecting.
#define CLIENT_PORT 8512
// Max number of bytes we receive in one recv() call.
#define BUFFER_SIZE 1024

/*
 * Print usage instructions.
 * argv[0] usually contains executable name, so we reuse it in help text.
 */
static void usage(const char *prog) {
    fprintf(stderr,
            "Usage:\n"
            "  %s server\n"
            "  %s client <server_ip> <message>\n\n"
            "Ports:\n"
            "  Server listens on %d\n"
            "  Client binds on %d\n",
            prog, prog, SERVER_PORT, CLIENT_PORT);
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
static int run_server(void) {
    // File descriptor for listening socket. -1 means “not created/open yet”.
    int server_fd = -1;
    // File descriptor returned by accept() for the connected client.
    int conn_fd = -1;

    // Address struct describing where server binds.
    struct sockaddr_in server_addr;
    // Address struct filled by accept() with remote client endpoint.
    struct sockaddr_in client_addr;
    // Must contain size of client_addr when passed to accept().
    socklen_t client_len = sizeof(client_addr);

    // Receive buffer (+1 will be reserved for '\0').
    char buffer[BUFFER_SIZE];

    // AF_INET = IPv4, SOCK_STREAM = TCP stream socket.
    server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd < 0) {
        perror("socket");
        return 1;
    }

    // SO_REUSEADDR helps avoid "Address already in use" after quick restarts.
    int opt = 1;
    if (setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) < 0) {
        perror("setsockopt");
        close(server_fd);
        return 1;
    }

    // Always zero structs before filling selected fields.
    memset(&server_addr, 0, sizeof(server_addr));
    server_addr.sin_family = AF_INET;
    // INADDR_ANY means “accept connections on all local interfaces”.
    server_addr.sin_addr.s_addr = htonl(INADDR_ANY);
    // Ports in sockaddr_in must be in network byte order.
    server_addr.sin_port = htons(SERVER_PORT);

    // Associate socket fd with local ip:port.
    if (bind(server_fd, (struct sockaddr *)&server_addr, sizeof(server_addr)) < 0) {
        perror("bind");
        close(server_fd);
        return 1;
    }

    // Start passive listening. Backlog=1 is enough for this small example.
    if (listen(server_fd, 1) < 0) {
        perror("listen");
        close(server_fd);
        return 1;
    }

    printf("Server listening on port %d...\n", SERVER_PORT);

    // Block until one client connects.
    conn_fd = accept(server_fd, (struct sockaddr *)&client_addr, &client_len);
    if (conn_fd < 0) {
        perror("accept");
        close(server_fd);
        return 1;
    }

    // Read bytes from client. recv returns byte count, 0, or -1 on error.
    ssize_t bytes_read = recv(conn_fd, buffer, sizeof(buffer) - 1, 0);
    if (bytes_read < 0) {
        perror("recv");
        close(conn_fd);
        close(server_fd);
        return 1;
    }

    // Make received bytes printable as a C string.
    buffer[bytes_read] = '\0';

    // Convert client address from binary to printable dotted-decimal text.
    char client_ip[INET_ADDRSTRLEN] = {0};
    inet_ntop(AF_INET, &client_addr.sin_addr, client_ip, sizeof(client_ip));

    printf("Received from %s:%d -> %s\n",
           client_ip,
           ntohs(client_addr.sin_port), // client source port (host byte order)
           buffer);

    // Cleanup descriptors when done.
    close(conn_fd);
    close(server_fd);
    return 0;
}

/*
 * Run client mode:
 * 1) Create TCP socket.
 * 2) Bind client side to fixed local port 8512.
 * 3) Parse destination IP.
 * 4) Connect to server on port 8513.
 * 5) Send message bytes.
 */
static int run_client(const char *server_ip, const char *message) {
    int sock_fd = -1;
    struct sockaddr_in local_addr;
    struct sockaddr_in server_addr;

    sock_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (sock_fd < 0) {
        perror("socket");
        return 1;
    }

    int opt = 1;
    if (setsockopt(sock_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) < 0) {
        perror("setsockopt");
        close(sock_fd);
        return 1;
    }

    // Local side endpoint for this client socket.
    memset(&local_addr, 0, sizeof(local_addr));
    local_addr.sin_family = AF_INET;
    local_addr.sin_addr.s_addr = htonl(INADDR_ANY);
    local_addr.sin_port = htons(CLIENT_PORT);

    // Explicit bind makes source port deterministic (8512) for learning/demo.
    if (bind(sock_fd, (struct sockaddr *)&local_addr, sizeof(local_addr)) < 0) {
        perror("bind");
        close(sock_fd);
        return 1;
    }

    // Remote server endpoint.
    memset(&server_addr, 0, sizeof(server_addr));
    server_addr.sin_family = AF_INET;
    server_addr.sin_port = htons(SERVER_PORT);

    // Convert input string like "127.0.0.1" to binary network address.
    if (inet_pton(AF_INET, server_ip, &server_addr.sin_addr) <= 0) {
        fprintf(stderr, "Invalid server IP: %s\n", server_ip);
        close(sock_fd);
        return 1;
    }

    // Perform TCP 3-way handshake with the server.
    if (connect(sock_fd, (struct sockaddr *)&server_addr, sizeof(server_addr)) < 0) {
        perror("connect");
        close(sock_fd);
        return 1;
    }

    // Send the message exactly as provided.
    size_t msg_len = strlen(message);
    if (send(sock_fd, message, msg_len, 0) != (ssize_t)msg_len) {
        perror("send");
        close(sock_fd);
        return 1;
    }

    printf("Sent message to %s:%d from local port %d\n", server_ip, SERVER_PORT, CLIENT_PORT);

    close(sock_fd);
    return 0;
}

/*
 * Entry point:
 * - `server` mode waits for one message.
 * - `client <ip> <message>` connects and sends one message.
 */
int main(int argc, char *argv[]) {
    if (argc < 2) {
        usage(argv[0]);
        return 1;
    }

    if (strcmp(argv[1], "server") == 0) {
        return run_server();
    }

    if (strcmp(argv[1], "client") == 0) {
        if (argc < 4) {
            usage(argv[0]);
            return 1;
        }
        return run_client(argv[2], argv[3]);
    }

    usage(argv[0]);
    return 1;
}
