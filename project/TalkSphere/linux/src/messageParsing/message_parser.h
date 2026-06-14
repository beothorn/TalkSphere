#ifndef TALKSPHERE_MESSAGE_PARSER_H
#define TALKSPHERE_MESSAGE_PARSER_H

#include <stddef.h>

struct message_processing_dependencies {
    int (*send_message_to_endpoint)(
        const char *remote_host,
        int remote_port,
        const char *message_text
    );
    int (*send_message_to_endpoint_with_response)(
        const char *remote_host,
        int remote_port,
        const char *message_text,
        char *response_text,
        size_t response_text_size
    );
    int listening_port;
    int peer_port;
    const char *app_storage_directory_path;
};

int process_received_message(
    const char *message_text,
    const struct message_processing_dependencies *message_processing_dependencies,
    char *response_text,
    size_t response_text_size
);

#endif
