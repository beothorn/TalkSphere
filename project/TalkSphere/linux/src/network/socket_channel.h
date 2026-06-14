#ifndef TALKSPHERE_SOCKET_BASICS_H
#define TALKSPHERE_SOCKET_BASICS_H

#include <stddef.h>

int run_socket_channel(
    int client_port,
    int server_port,
    const char *app_storage_directory_path
);

int request_remote_offerings_through_client_port(
    int local_client_port,
    char *offerings_text,
    size_t offerings_text_size
);

int request_message_send_through_client_port(
    int local_client_port,
    const char *message_text
);

#endif
