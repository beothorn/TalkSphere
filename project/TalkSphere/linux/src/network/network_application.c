#include "network_application.h"

#include "socket_channel.h"

#include "../common/result.h"
#include "../logging.h"

#include <stdio.h>

#define OFFERINGS_TEXT_SIZE 8192

static int print_placeholder(
    const char *placeholder_text
) {
    LOG_TRACE("print_placeholder(): now network reports a command that exists before the feature body is implemented");

    printf(
        "%s\n",
        placeholder_text
    );
    return TALKSPHERE_SUCCESS;
}

int network_application_run_server(
    int listen_port,
    int peer_port,
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("network_application_run_server(): now we start the socket channel for the run command");

    return run_socket_channel(
        listen_port,
        peer_port,
        resolved_storage_directory_path
    );
}

int network_application_print_connected_offerings(
    int local_client_port
) {
    LOG_TRACE("network_application_print_connected_offerings(): now we ask a running local instance to fetch its connected peer offerings");

    char offerings_text[OFFERINGS_TEXT_SIZE];
    if (request_remote_offerings_through_client_port(
            local_client_port,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "%s",
        offerings_text
    );
    return TALKSPHERE_SUCCESS;
}

int network_application_send_connected_message(
    int local_client_port,
    const char *message_text
) {
    LOG_TRACE("network_application_send_connected_message(): now we ask a running local instance to send a message to its connected peer");

    return request_message_send_through_client_port(
        local_client_port,
        message_text
    );
}

int network_application_print_run_server_dry_run(
    int listen_port,
    int peer_port,
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("network_application_print_run_server_dry_run(): now we describe server startup without changing state");

    printf(
        "Would run TalkSphere with listen port %d, peer port %d, home folder %s\n",
        listen_port,
        peer_port,
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}

int network_application_print_ping_dry_run(
    const char *network_address_text
) {
    LOG_TRACE("network_application_print_ping_dry_run(): now we describe peer ping without changing state");

    printf(
        "Would ping TalkSphere peer %s\n",
        network_address_text
    );
    return TALKSPHERE_SUCCESS;
}

int network_application_print_remote_offerings_dry_run(
    const char *network_address_text
) {
    LOG_TRACE("network_application_print_remote_offerings_dry_run(): now we describe remote offerings lookup without changing state");

    printf(
        "Would print offerings from peer %s\n",
        network_address_text
    );
    return TALKSPHERE_SUCCESS;
}

int network_application_print_connected_offerings_dry_run(
    int local_client_port
) {
    LOG_TRACE("network_application_print_connected_offerings_dry_run(): now we describe connected peer offerings lookup without changing state");

    printf(
        "Would ask local TalkSphere client port %d for connected peer offerings\n",
        local_client_port
    );
    return TALKSPHERE_SUCCESS;
}

int network_application_print_send_message_dry_run(
    int local_client_port,
    const char *message_text
) {
    LOG_TRACE("network_application_print_send_message_dry_run(): now we describe connected peer message sending without changing state");

    printf(
        "Would ask local TalkSphere client port %d to send message: %s\n",
        local_client_port,
        message_text
    );
    return TALKSPHERE_SUCCESS;
}

int network_application_print_ping_placeholder(void) {
    LOG_TRACE("network_application_print_ping_placeholder(): now we report that network ping is not implemented yet");

    return print_placeholder("network ping is not implemented yet");
}

int network_application_print_remote_offerings_placeholder(void) {
    LOG_TRACE("network_application_print_remote_offerings_placeholder(): now we report that remote offerings lookup is not implemented yet");

    return print_placeholder("remote offerings lookup is not implemented yet");
}
