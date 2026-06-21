#ifndef TALKSPHERE_NETWORK_APPLICATION_H
#define TALKSPHERE_NETWORK_APPLICATION_H

int network_application_run_server(
    int listen_port,
    int peer_port,
    const char *resolved_storage_directory_path
);

int network_application_print_connected_offerings(
    int local_client_port
);

int network_application_send_connected_message(
    int local_client_port,
    const char *message_text
);

int network_application_print_run_server_dry_run(
    int listen_port,
    int peer_port,
    const char *resolved_storage_directory_path
);

int network_application_print_ping_dry_run(
    const char *network_address_text
);

int network_application_print_remote_offerings_dry_run(
    const char *network_address_text
);

int network_application_print_connected_offerings_dry_run(
    int local_client_port
);

int network_application_print_send_message_dry_run(
    int local_client_port,
    const char *message_text
);

int network_application_print_ping_placeholder(void);

int network_application_print_remote_offerings_placeholder(void);

#endif
