#include <arpa/inet.h>
#include <errno.h>
#include <netinet/in.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

#define SERVER_PORT 8513
#define CLIENT_PORT 8512
#define BUFFER_SIZE 1024

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

static int run_server(void) {
    int server_fd = -1;
    int conn_fd = -1;
    struct sockaddr_in server_addr;
    struct sockaddr_in client_addr;
    socklen_t client_len = sizeof(client_addr);
    char buffer[BUFFER_SIZE];

    server_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (server_fd < 0) {
        perror("socket");
        return 1;
    }

    int opt = 1;
    if (setsockopt(server_fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt)) < 0) {
        perror("setsockopt");
        close(server_fd);
        return 1;
    }

    memset(&server_addr, 0, sizeof(server_addr));
    server_addr.sin_family = AF_INET;
    server_addr.sin_addr.s_addr = htonl(INADDR_ANY);
    server_addr.sin_port = htons(SERVER_PORT);

    if (bind(server_fd, (struct sockaddr *)&server_addr, sizeof(server_addr)) < 0) {
        perror("bind");
        close(server_fd);
        return 1;
    }

    if (listen(server_fd, 1) < 0) {
        perror("listen");
        close(server_fd);
        return 1;
    }

    printf("Server listening on port %d...\n", SERVER_PORT);

    conn_fd = accept(server_fd, (struct sockaddr *)&client_addr, &client_len);
    if (conn_fd < 0) {
        perror("accept");
        close(server_fd);
        return 1;
    }

    ssize_t bytes_read = recv(conn_fd, buffer, sizeof(buffer) - 1, 0);
    if (bytes_read < 0) {
        perror("recv");
        close(conn_fd);
        close(server_fd);
        return 1;
    }

    buffer[bytes_read] = '\0';

    char client_ip[INET_ADDRSTRLEN] = {0};
    inet_ntop(AF_INET, &client_addr.sin_addr, client_ip, sizeof(client_ip));
    printf("Received from %s:%d -> %s\n",
           client_ip,
           ntohs(client_addr.sin_port),
           buffer);

    close(conn_fd);
    close(server_fd);
    return 0;
}

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

    memset(&local_addr, 0, sizeof(local_addr));
    local_addr.sin_family = AF_INET;
    local_addr.sin_addr.s_addr = htonl(INADDR_ANY);
    local_addr.sin_port = htons(CLIENT_PORT);

    if (bind(sock_fd, (struct sockaddr *)&local_addr, sizeof(local_addr)) < 0) {
        perror("bind");
        close(sock_fd);
        return 1;
    }

    memset(&server_addr, 0, sizeof(server_addr));
    server_addr.sin_family = AF_INET;
    server_addr.sin_port = htons(SERVER_PORT);

    if (inet_pton(AF_INET, server_ip, &server_addr.sin_addr) <= 0) {
        fprintf(stderr, "Invalid server IP: %s\n", server_ip);
        close(sock_fd);
        return 1;
    }

    if (connect(sock_fd, (struct sockaddr *)&server_addr, sizeof(server_addr)) < 0) {
        perror("connect");
        close(sock_fd);
        return 1;
    }

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
