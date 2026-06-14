#include "program_arguments.h"

#include "logging.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define PROGRAM_NAME_ARGUMENT_INDEX 0
#define FIRST_COMMAND_ARGUMENT_INDEX 1
#define FIRST_ARGUMENT_AFTER_DRY_RUN_INDEX 2
#define RUN_LISTEN_PORT_ARGUMENT_OFFSET 1
#define RUN_PEER_PORT_ARGUMENT_OFFSET 2
#define RUN_HOME_FOLDER_ARGUMENT_OFFSET 3
#define CONFIG_FIRST_CHILD_ARGUMENT_OFFSET 1
#define CONFIG_SECOND_CHILD_ARGUMENT_OFFSET 2
#define ENCRYPTION_CHILD_ARGUMENT_OFFSET 1
#define ENCRYPTION_MESSAGE_ARGUMENT_OFFSET 2
#define LEDGER_CHILD_ARGUMENT_OFFSET 1
#define NETWORK_CHILD_ARGUMENT_OFFSET 1
#define NETWORK_ADDRESS_ARGUMENT_OFFSET 2
#define OFFERINGS_CHILD_ARGUMENT_OFFSET 1
#define OFFERINGS_VALUE_ARGUMENT_OFFSET 2
#define SHARE_SCOPE_ARGUMENT_OFFSET 1
#define SHARE_CHILD_ARGUMENT_OFFSET 2
#define COMMAND_WITH_NO_CHILD_COUNT 1
#define COMMAND_WITH_ONE_CHILD_COUNT 2
#define COMMAND_WITH_TWO_CHILDREN_COUNT 3
#define COMMAND_WITH_THREE_CHILDREN_COUNT 4
#define DECIMAL_BASE 10
#define MINIMUM_PORT 1
#define MAXIMUM_PORT 65535
#define STRING_TERMINATOR '\0'

#define RUN_COMMAND_TEXT "run"
#define CONFIG_COMMAND_TEXT "config"
#define ENCRYPTION_COMMAND_TEXT "encryption"
#define FILES_COMMAND_TEXT "files"
#define LEDGER_COMMAND_TEXT "ledger"
#define NETWORK_COMMAND_TEXT "network"
#define OFFERINGS_COMMAND_TEXT "offerings"
#define SHARE_COMMAND_TEXT "share"
#define GET_COMMAND_TEXT "get"
#define HOME_COMMAND_TEXT "home"
#define CREATE_COMMAND_TEXT "create"
#define RECREATE_COMMAND_TEXT "recreate"
#define ENCRYPT_MESSAGE_COMMAND_TEXT "encrypt_message"
#define SIGN_MESSAGE_COMMAND_TEXT "sign_message"
#define CREDIT_SUMMARY_COMMAND_TEXT "credit_summary"
#define PING_COMMAND_TEXT "ping"
#define ADD_COMMAND_TEXT "add"
#define EDIT_COMMAND_TEXT "edit"
#define REMOVE_COMMAND_TEXT "remove"
#define LOCAL_COMMAND_TEXT "local"
#define REMOTE_COMMAND_TEXT "remote"
#define LIST_COMMAND_TEXT "ls"
#define HELP_LONG_ARGUMENT_TEXT "--help"
#define HELP_WORD_ARGUMENT_TEXT "help"
#define HELP_SHORT_ARGUMENT_TEXT "h"
#define DRY_RUN_LONG_ARGUMENT_TEXT "--dry-run"
#define DRY_RUN_SHORT_ARGUMENT_TEXT "d"

static int argument_text_is(
    const char *argument_text,
    const char *expected_argument_text
) {
    LOG_TRACE("argument_text_is(): now we compare a command line word with an expected command word");

    return strcmp(
        argument_text,
        expected_argument_text
    ) == 0;
}

static int argument_is_help(
    const char *argument_text
) {
    LOG_TRACE("argument_is_help(): now we check whether this command word asks for help");

    return argument_text_is(
        argument_text,
        HELP_LONG_ARGUMENT_TEXT
    )
        || argument_text_is(
            argument_text,
            HELP_WORD_ARGUMENT_TEXT
        )
        || argument_text_is(
            argument_text,
            HELP_SHORT_ARGUMENT_TEXT
        );
}

static int argument_is_dry_run(
    const char *argument_text
) {
    LOG_TRACE("argument_is_dry_run(): now we check whether this command word asks to print the operation");

    return argument_text_is(
        argument_text,
        DRY_RUN_LONG_ARGUMENT_TEXT
    )
        || argument_text_is(
            argument_text,
            DRY_RUN_SHORT_ARGUMENT_TEXT
        );
}

static void print_main_help(
    FILE *output_file,
    const char *program_name
) {
    LOG_TRACE("print_main_help(): now we print the complete command overview");

    fprintf(
        output_file,
        "TalkSphere command line\n\n"
        "Usage:\n"
        "  %s [--dry-run|d] <command> [arguments]\n"
        "  %s [--help|help|h]\n\n"
        "Commands:\n"
        "  run <listen_port> <peer_port> [home_folder]\n"
        "      Start the TalkSphere socket service and connect to the peer port.\n"
        "  config get home\n"
        "      Print the resolved home folder.\n"
        "  encryption [--help|help|h]\n"
        "      Show encryption commands.\n"
        "  files home\n"
        "      Print the resolved home folder.\n"
        "  ledger [--help|help|h]\n"
        "      Show ledger commands.\n"
        "  network [--help|help|h]\n"
        "      Show network commands.\n"
        "  offerings [--help|help|h]\n"
        "      Show offering commands.\n"
        "  share [--help|help|h]\n"
        "      Show shared storage commands.\n\n"
        "Dry run:\n"
        "  Add --dry-run or d before the command to print what would happen.\n",
        program_name,
        program_name
    );
}

static void print_encryption_help(
    FILE *output_file,
    const char *program_name
) {
    LOG_TRACE("print_encryption_help(): now we print the encryption command help");

    fprintf(
        output_file,
        "Encryption commands\n\n"
        "Usage:\n"
        "  %s [--dry-run|d] encryption create\n"
        "      Create encryption keys in the home folder and fail if they already exist.\n"
        "  %s [--dry-run|d] encryption recreate\n"
        "      Replace encryption keys in the home folder and fail if they do not exist.\n"
        "  %s [--dry-run|d] encryption encrypt_message \"message\"\n"
        "      Print the encrypted message to stdout.\n"
        "  %s [--dry-run|d] encryption sign_message \"message\"\n"
        "      Print the message signature to stdout.\n",
        program_name,
        program_name,
        program_name,
        program_name
    );
}

static void print_ledger_help(
    FILE *output_file,
    const char *program_name
) {
    LOG_TRACE("print_ledger_help(): now we print the ledger command help");

    fprintf(
        output_file,
        "Ledger commands\n\n"
        "Usage:\n"
        "  %s [--dry-run|d] ledger credit_summary\n"
        "      Print credit totals from the local ledger.\n",
        program_name
    );
}

static void print_config_help(
    FILE *output_file,
    const char *program_name
) {
    LOG_TRACE("print_config_help(): now we print the configuration command help");

    fprintf(
        output_file,
        "Config commands\n\n"
        "Usage:\n"
        "  %s [--dry-run|d] config get home\n"
        "      Print the resolved home folder.\n",
        program_name
    );
}

static void print_network_help(
    FILE *output_file,
    const char *program_name
) {
    LOG_TRACE("print_network_help(): now we print the network command help");

    fprintf(
        output_file,
        "Network commands\n\n"
        "Usage:\n"
        "  %s [--dry-run|d] network ping <ip:port>\n"
        "      Send a TalkSphere ping to check whether a peer is reachable.\n",
        program_name
    );
}

static void print_offerings_help(
    FILE *output_file,
    const char *program_name
) {
    LOG_TRACE("print_offerings_help(): now we print the offerings command help");

    fprintf(
        output_file,
        "Offerings commands\n\n"
        "Usage:\n"
        "  %s [--dry-run|d] offerings <ip:port>\n"
        "      Print offerings exposed by a remote peer.\n"
        "  %s [--dry-run|d] offerings get\n"
        "      Print local offerings.\n"
        "  %s [--dry-run|d] offerings add <offering options>\n"
        "      Add a local offering.\n"
        "  %s [--dry-run|d] offerings edit <offering options>\n"
        "      Edit a local offering.\n"
        "  %s [--dry-run|d] offerings remove <offering>\n"
        "      Remove a local offering.\n",
        program_name,
        program_name,
        program_name,
        program_name,
        program_name
    );
}

static void print_share_help(
    FILE *output_file,
    const char *program_name
) {
    LOG_TRACE("print_share_help(): now we print the shared storage command help");

    fprintf(
        output_file,
        "Shared storage commands\n\n"
        "Usage:\n"
        "  %s [--dry-run|d] share local ls\n"
        "      List local shared files with metadata.\n"
        "  %s [--dry-run|d] share remote ls\n"
        "      List remote shared files with metadata.\n",
        program_name,
        program_name
    );
}

static void print_help_for_mode(
    FILE *output_file,
    const char *program_name,
    enum program_mode program_mode
) {
    LOG_TRACE("print_help_for_mode(): now we route help output to the requested command family");

    if (program_mode == PROGRAM_MODE_PRINT_CONFIG_HELP) {
        print_config_help(
            output_file,
            program_name
        );
    } else if (program_mode == PROGRAM_MODE_PRINT_ENCRYPTION_HELP) {
        print_encryption_help(
            output_file,
            program_name
        );
    } else if (program_mode == PROGRAM_MODE_PRINT_LEDGER_HELP) {
        print_ledger_help(
            output_file,
            program_name
        );
    } else if (program_mode == PROGRAM_MODE_PRINT_NETWORK_HELP) {
        print_network_help(
            output_file,
            program_name
        );
    } else if (program_mode == PROGRAM_MODE_PRINT_OFFERINGS_HELP) {
        print_offerings_help(
            output_file,
            program_name
        );
    } else if (program_mode == PROGRAM_MODE_PRINT_SHARE_HELP) {
        print_share_help(
            output_file,
            program_name
        );
    } else {
        print_main_help(
            output_file,
            program_name
        );
    }
}

static int parse_port(
    const char *port_text,
    const char *port_name,
    int *port
) {
    LOG_TRACE("parse_port(): now we validate that the port text is a number in the TCP port range");
    LOG_DEBUG(
        "Parsing %s port from text %s",
        port_name,
        port_text
    );

    char *end_character = NULL;
    long port_value = strtol(
        port_text,
        &end_character,
        DECIMAL_BASE
    );

    if (port_text[0] == STRING_TERMINATOR
        || *end_character != STRING_TERMINATOR
        || port_value < MINIMUM_PORT
        || port_value > MAXIMUM_PORT
    ) {
        LOG_WARN("The port is unwanted because it must be an integer inside the TCP port range");
        fprintf(
            stderr,
            "Invalid %s port: %s\n",
            port_name,
            port_text
        );
        return TALKSPHERE_FAILURE;
    }

    *port = (int)port_value;
    return TALKSPHERE_SUCCESS;
}

static int validate_different_ports(
    const struct program_arguments *program_arguments
) {
    LOG_TRACE("validate_different_ports(): now we check the listener and peer ports are not the same");

    if (program_arguments->listen_port == program_arguments->peer_port) {
        LOG_WARN("Listen and peer ports are unwanted when equal because the local socket cannot be its own peer");
        fprintf(
            stderr,
            "Listen and peer ports must be different.\n"
        );
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static void initialize_program_arguments(
    struct program_arguments *program_arguments
) {
    LOG_TRACE("initialize_program_arguments(): now we set command defaults before applying user arguments");

    program_arguments->listen_port = DEFAULT_SERVER_PORT;
    program_arguments->peer_port = DEFAULT_CLIENT_PORT;
    program_arguments->app_storage_directory_path = NULL;
    program_arguments->message_text = NULL;
    program_arguments->network_address_text = NULL;
    program_arguments->offering_text = NULL;
    program_arguments->dry_run_is_enabled = 0;
    program_arguments->program_mode = PROGRAM_MODE_PRINT_MAIN_HELP;
}

static int parse_run_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_run_command(): now we parse the command that starts the TalkSphere service");

    if (command_argument_count != COMMAND_WITH_TWO_CHILDREN_COUNT
        && command_argument_count != COMMAND_WITH_THREE_CHILDREN_COUNT
    ) {
        LOG_WARN("Run arguments are unwanted because run needs a listen port, a peer port, and optional home folder");
        return TALKSPHERE_FAILURE;
    }

    if (parse_port(
            command_arguments[RUN_LISTEN_PORT_ARGUMENT_OFFSET],
            "listen",
            &program_arguments->listen_port
        ) != TALKSPHERE_SUCCESS
        || parse_port(
            command_arguments[RUN_PEER_PORT_ARGUMENT_OFFSET],
            "peer",
            &program_arguments->peer_port
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (command_argument_count == COMMAND_WITH_THREE_CHILDREN_COUNT) {
        program_arguments->app_storage_directory_path =
            command_arguments[RUN_HOME_FOLDER_ARGUMENT_OFFSET];
    }

    program_arguments->program_mode = PROGRAM_MODE_RUN_SERVER;
    return validate_different_ports(program_arguments);
}

static int parse_config_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_config_command(): now we parse configuration commands");

    if (command_argument_count == COMMAND_WITH_NO_CHILD_COUNT
        || (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
            && argument_is_help(command_arguments[CONFIG_FIRST_CHILD_ARGUMENT_OFFSET]))
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_CONFIG_HELP;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[CONFIG_FIRST_CHILD_ARGUMENT_OFFSET],
            GET_COMMAND_TEXT
        )
        && argument_text_is(
            command_arguments[CONFIG_SECOND_CHILD_ARGUMENT_OFFSET],
            HOME_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_HOME;
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("Config arguments are unwanted because this command only supports get home");
    return TALKSPHERE_FAILURE;
}

static int parse_files_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_files_command(): now we parse file commands");

    if (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
        && argument_text_is(
            command_arguments[CONFIG_FIRST_CHILD_ARGUMENT_OFFSET],
            HOME_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_HOME;
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("Files arguments are unwanted because this command only supports home");
    return TALKSPHERE_FAILURE;
}

static int parse_encryption_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_encryption_command(): now we parse encryption commands");

    if (command_argument_count == COMMAND_WITH_NO_CHILD_COUNT
        || (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
            && argument_is_help(command_arguments[ENCRYPTION_CHILD_ARGUMENT_OFFSET]))
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_ENCRYPTION_HELP;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
        && argument_text_is(
            command_arguments[ENCRYPTION_CHILD_ARGUMENT_OFFSET],
            CREATE_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_CREATE_ENCRYPTION_KEYS;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
        && argument_text_is(
            command_arguments[ENCRYPTION_CHILD_ARGUMENT_OFFSET],
            RECREATE_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_RECREATE_ENCRYPTION_KEYS;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[ENCRYPTION_CHILD_ARGUMENT_OFFSET],
            ENCRYPT_MESSAGE_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_ENCRYPT_MESSAGE;
        program_arguments->message_text = command_arguments[ENCRYPTION_MESSAGE_ARGUMENT_OFFSET];
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[ENCRYPTION_CHILD_ARGUMENT_OFFSET],
            SIGN_MESSAGE_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_SIGN_MESSAGE;
        program_arguments->message_text = command_arguments[ENCRYPTION_MESSAGE_ARGUMENT_OFFSET];
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("Encryption arguments are unwanted because the command shape is not supported");
    return TALKSPHERE_FAILURE;
}

static int parse_ledger_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_ledger_command(): now we parse ledger commands");

    if (command_argument_count == COMMAND_WITH_NO_CHILD_COUNT
        || (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
            && argument_is_help(command_arguments[LEDGER_CHILD_ARGUMENT_OFFSET]))
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_LEDGER_HELP;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
        && argument_text_is(
            command_arguments[LEDGER_CHILD_ARGUMENT_OFFSET],
            CREDIT_SUMMARY_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_LEDGER_SUMMARY;
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("Ledger arguments are unwanted because the command shape is not supported");
    return TALKSPHERE_FAILURE;
}

static int parse_network_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_network_command(): now we parse network commands");

    if (command_argument_count == COMMAND_WITH_NO_CHILD_COUNT
        || (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
            && argument_is_help(command_arguments[NETWORK_CHILD_ARGUMENT_OFFSET]))
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_NETWORK_HELP;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[NETWORK_CHILD_ARGUMENT_OFFSET],
            PING_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PING_NETWORK_PEER;
        program_arguments->network_address_text = command_arguments[NETWORK_ADDRESS_ARGUMENT_OFFSET];
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("Network arguments are unwanted because the command shape is not supported");
    return TALKSPHERE_FAILURE;
}

static int parse_offerings_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_offerings_command(): now we parse offerings commands");

    if (command_argument_count == COMMAND_WITH_NO_CHILD_COUNT
        || (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
            && argument_is_help(command_arguments[OFFERINGS_CHILD_ARGUMENT_OFFSET]))
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_OFFERINGS_HELP;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
        && argument_text_is(
            command_arguments[OFFERINGS_CHILD_ARGUMENT_OFFSET],
            GET_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_LOCAL_OFFERINGS;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_REMOTE_OFFERINGS;
        program_arguments->network_address_text = command_arguments[OFFERINGS_CHILD_ARGUMENT_OFFSET];
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[OFFERINGS_CHILD_ARGUMENT_OFFSET],
            ADD_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_ADD_OFFERING;
        program_arguments->offering_text = command_arguments[OFFERINGS_VALUE_ARGUMENT_OFFSET];
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[OFFERINGS_CHILD_ARGUMENT_OFFSET],
            EDIT_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_EDIT_OFFERING;
        program_arguments->offering_text = command_arguments[OFFERINGS_VALUE_ARGUMENT_OFFSET];
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[OFFERINGS_CHILD_ARGUMENT_OFFSET],
            REMOVE_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_REMOVE_OFFERING;
        program_arguments->offering_text = command_arguments[OFFERINGS_VALUE_ARGUMENT_OFFSET];
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("Offerings arguments are unwanted because the command shape is not supported");
    return TALKSPHERE_FAILURE;
}

static int parse_share_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_share_command(): now we parse shared storage commands");

    if (command_argument_count == COMMAND_WITH_NO_CHILD_COUNT
        || (command_argument_count == COMMAND_WITH_ONE_CHILD_COUNT
            && argument_is_help(command_arguments[SHARE_SCOPE_ARGUMENT_OFFSET]))
    ) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_SHARE_HELP;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[SHARE_SCOPE_ARGUMENT_OFFSET],
            LOCAL_COMMAND_TEXT
        )
        && argument_text_is(
            command_arguments[SHARE_CHILD_ARGUMENT_OFFSET],
            LIST_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_LIST_LOCAL_SHARED_STORAGE;
        return TALKSPHERE_SUCCESS;
    }

    if (command_argument_count == COMMAND_WITH_TWO_CHILDREN_COUNT
        && argument_text_is(
            command_arguments[SHARE_SCOPE_ARGUMENT_OFFSET],
            REMOTE_COMMAND_TEXT
        )
        && argument_text_is(
            command_arguments[SHARE_CHILD_ARGUMENT_OFFSET],
            LIST_COMMAND_TEXT
        )
    ) {
        program_arguments->program_mode = PROGRAM_MODE_LIST_REMOTE_SHARED_STORAGE;
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("Share arguments are unwanted because the command shape is not supported");
    return TALKSPHERE_FAILURE;
}

static int parse_command(
    int command_argument_count,
    char *command_arguments[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_command(): now we route the first command word to a domain parser");

    if (command_argument_count == 0) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_MAIN_HELP;
        return TALKSPHERE_SUCCESS;
    }

    if (argument_is_help(command_arguments[0])) {
        program_arguments->program_mode = PROGRAM_MODE_PRINT_MAIN_HELP;
        return command_argument_count == COMMAND_WITH_NO_CHILD_COUNT
            ? TALKSPHERE_SUCCESS
            : TALKSPHERE_FAILURE;
    }

    if (argument_text_is(
            command_arguments[0],
            RUN_COMMAND_TEXT
        )
    ) {
        return parse_run_command(
            command_argument_count,
            command_arguments,
            program_arguments
        );
    }

    if (argument_text_is(
            command_arguments[0],
            CONFIG_COMMAND_TEXT
        )
    ) {
        return parse_config_command(
            command_argument_count,
            command_arguments,
            program_arguments
        );
    }

    if (argument_text_is(
            command_arguments[0],
            FILES_COMMAND_TEXT
        )
    ) {
        return parse_files_command(
            command_argument_count,
            command_arguments,
            program_arguments
        );
    }

    if (argument_text_is(
            command_arguments[0],
            ENCRYPTION_COMMAND_TEXT
        )
    ) {
        return parse_encryption_command(
            command_argument_count,
            command_arguments,
            program_arguments
        );
    }

    if (argument_text_is(
            command_arguments[0],
            LEDGER_COMMAND_TEXT
        )
    ) {
        return parse_ledger_command(
            command_argument_count,
            command_arguments,
            program_arguments
        );
    }

    if (argument_text_is(
            command_arguments[0],
            NETWORK_COMMAND_TEXT
        )
    ) {
        return parse_network_command(
            command_argument_count,
            command_arguments,
            program_arguments
        );
    }

    if (argument_text_is(
            command_arguments[0],
            OFFERINGS_COMMAND_TEXT
        )
    ) {
        return parse_offerings_command(
            command_argument_count,
            command_arguments,
            program_arguments
        );
    }

    if (argument_text_is(
            command_arguments[0],
            SHARE_COMMAND_TEXT
        )
    ) {
        return parse_share_command(
            command_argument_count,
            command_arguments,
            program_arguments
        );
    }

    LOG_WARN("The command is unwanted because TalkSphere does not know that command family");
    return TALKSPHERE_FAILURE;
}

int parse_program_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_program_arguments(): now we turn process arguments into a command that main can execute");
    LOG_DEBUG(
        "Received %d program arguments",
        argument_count
    );

    const char *program_name = argument_values[PROGRAM_NAME_ARGUMENT_INDEX];
    initialize_program_arguments(program_arguments);

    int first_command_argument_index = FIRST_COMMAND_ARGUMENT_INDEX;
    if (argument_count > FIRST_COMMAND_ARGUMENT_INDEX
        && argument_is_dry_run(argument_values[FIRST_COMMAND_ARGUMENT_INDEX])
    ) {
        program_arguments->dry_run_is_enabled = 1;
        first_command_argument_index = FIRST_ARGUMENT_AFTER_DRY_RUN_INDEX;
    }

    int command_argument_count = argument_count - first_command_argument_index;
    char **command_arguments = &argument_values[first_command_argument_index];

    int parse_result = parse_command(
        command_argument_count,
        command_arguments,
        program_arguments
    );

    if (parse_result != TALKSPHERE_SUCCESS) {
        print_main_help(
            stderr,
            program_name
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_PRINT_MAIN_HELP
        || program_arguments->program_mode == PROGRAM_MODE_PRINT_CONFIG_HELP
        || program_arguments->program_mode == PROGRAM_MODE_PRINT_ENCRYPTION_HELP
        || program_arguments->program_mode == PROGRAM_MODE_PRINT_LEDGER_HELP
        || program_arguments->program_mode == PROGRAM_MODE_PRINT_NETWORK_HELP
        || program_arguments->program_mode == PROGRAM_MODE_PRINT_OFFERINGS_HELP
        || program_arguments->program_mode == PROGRAM_MODE_PRINT_SHARE_HELP
    ) {
        print_help_for_mode(
            stdout,
            program_name,
            program_arguments->program_mode
        );
    }

    return parse_result;
}
