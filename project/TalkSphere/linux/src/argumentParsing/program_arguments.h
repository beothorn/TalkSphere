#ifndef TALKSPHERE_PROGRAM_ARGUMENTS_H
#define TALKSPHERE_PROGRAM_ARGUMENTS_H

#include "../common/result.h"

#define DEFAULT_SERVER_PORT 8513
#define DEFAULT_CLIENT_PORT 8512

enum program_mode {
    PROGRAM_MODE_RUN_SERVER,
    PROGRAM_MODE_PRINT_MAIN_HELP,
    PROGRAM_MODE_PRINT_CONFIG_HELP,
    PROGRAM_MODE_PRINT_ENCRYPTION_HELP,
    PROGRAM_MODE_PRINT_LEDGER_HELP,
    PROGRAM_MODE_PRINT_NETWORK_HELP,
    PROGRAM_MODE_PRINT_OFFERINGS_HELP,
    PROGRAM_MODE_PRINT_TALK_HELP,
    PROGRAM_MODE_PRINT_SHARE_HELP,
    PROGRAM_MODE_PRINT_CREDIT_HELP,
    PROGRAM_MODE_PRINT_LEDGER_SUMMARY,
    PROGRAM_MODE_PRINT_HOME,
    PROGRAM_MODE_GET_CONFIG_VALUE,
    PROGRAM_MODE_SET_CONFIG_VALUE,
    PROGRAM_MODE_ADD_CONFIG_VALUE,
    PROGRAM_MODE_REMOVE_CONFIG_VALUE,
    PROGRAM_MODE_CREATE_ENCRYPTION_KEYS,
    PROGRAM_MODE_RECREATE_ENCRYPTION_KEYS,
    PROGRAM_MODE_ENCRYPT_MESSAGE,
    PROGRAM_MODE_SIGN_MESSAGE,
    PROGRAM_MODE_PING_NETWORK_PEER,
    PROGRAM_MODE_PRINT_REMOTE_OFFERINGS,
    PROGRAM_MODE_TALK_PRINT_REMOTE_OFFERINGS,
    PROGRAM_MODE_TALK_SEND_MESSAGE,
    PROGRAM_MODE_PRINT_LOCAL_OFFERINGS,
    PROGRAM_MODE_ADD_OFFERING,
    PROGRAM_MODE_EDIT_OFFERING,
    PROGRAM_MODE_REMOVE_OFFERING,
    PROGRAM_MODE_LIST_LOCAL_SHARED_STORAGE,
    PROGRAM_MODE_LIST_REMOTE_SHARED_STORAGE,
    PROGRAM_MODE_ADD_CREDIT_WITHDRAW_CODE,
    PROGRAM_MODE_REMOVE_CREDIT_WITHDRAW_CODE,
    PROGRAM_MODE_LIST_CREDIT_WITHDRAW_CODES
};

struct program_arguments {
    int listen_port;
    int peer_port;
    int local_client_port;
    const char *app_storage_directory_path;
    const char *message_text;
    const char *network_address_text;
    const char *offering_text;
    const char *config_key_text;
    const char *config_value_text;
    const char *credit_code_text;
    int credit_count;
    int dry_run_is_enabled;
    enum program_mode program_mode;
};

int parse_program_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
);

#endif
