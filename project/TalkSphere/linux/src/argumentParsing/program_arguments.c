#include "program_arguments.h"

#include "logging.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define PROGRAM_NAME_ARGUMENT_INDEX 0
#define FIRST_CHARACTER_INDEX 0
#define CLIENT_PORT_ARGUMENT_INDEX 1
#define SERVER_PORT_ARGUMENT_INDEX 2
#define STORAGE_DIRECTORY_ARGUMENT_INDEX 3
#define LEDGER_SUMMARY_ARGUMENT_INDEX 1
#define LEDGER_SUMMARY_STORAGE_DIRECTORY_ARGUMENT_INDEX 2
#define IDENTIFIER_ARGUMENT_INDEX 1
#define IDENTIFIER_STORAGE_DIRECTORY_ARGUMENT_INDEX 2
#define HOME_ARGUMENT_INDEX 1
#define HOME_STORAGE_DIRECTORY_ARGUMENT_INDEX 2
#define HELP_ARGUMENT_INDEX 1
#define DEFAULT_ARGUMENT_COUNT 1
#define CUSTOM_PORT_ARGUMENT_COUNT 3
#define CUSTOM_PORT_AND_STORAGE_ARGUMENT_COUNT 4
#define LEDGER_SUMMARY_ARGUMENT_COUNT 2
#define LEDGER_SUMMARY_WITH_STORAGE_ARGUMENT_COUNT 3
#define IDENTIFIER_ARGUMENT_COUNT 2
#define IDENTIFIER_WITH_STORAGE_ARGUMENT_COUNT 3
#define HOME_ARGUMENT_COUNT 2
#define HOME_WITH_STORAGE_ARGUMENT_COUNT 3
#define HELP_ARGUMENT_COUNT 2
#define LEDGER_SUMMARY_ARGUMENT_TEXT "--ledger-summary"
#define IDENTIFIER_ARGUMENT_TEXT "--id"
#define HOME_ARGUMENT_TEXT "--home"
#define HELP_ARGUMENT_TEXT "--help"

#define DECIMAL_BASE 10
#define MINIMUM_PORT 1
#define MAXIMUM_PORT 65535
#define STRING_TERMINATOR '\0'

static void print_usage(
    FILE *output_file,
    const char *program_name
) {
    LOG_TRACE("print_usage(): now we print the valid command shapes");

    fprintf(
        output_file,
        "Usage:\n"
        "  %s\n"
        "  %s <client_port> <server_port>\n"
        "  %s <client_port> <server_port> <storage_directory>\n"
        "  %s --id [storage_directory]\n"
        "  %s --home [storage_directory]\n"
        "  %s --ledger-summary [storage_directory]\n\n"
        "  %s --help\n\n"
        "Ports:\n"
        "  Default client port: %d\n"
        "  Default server port: %d\n",
        program_name,
        program_name,
        program_name,
        program_name,
        program_name,
        program_name,
        program_name,
        DEFAULT_CLIENT_PORT,
        DEFAULT_SERVER_PORT
    );
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

    if (port_text[FIRST_CHARACTER_INDEX] == STRING_TERMINATOR
        || *end_character != STRING_TERMINATOR
        || port_value < MINIMUM_PORT
        || port_value > MAXIMUM_PORT
    ) {
        LOG_WARN(
            "The %s port is unwanted because it must be an integer from %d to %d",
            port_name,
            MINIMUM_PORT,
            MAXIMUM_PORT
        );
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
    LOG_TRACE("validate_different_ports(): now we check the client and server do not compete for one port");

    if (program_arguments->client_port == program_arguments->server_port) {
        LOG_WARN("Client and server ports are unwanted when equal because both sockets need to bind");
        fprintf(
            stderr,
            "Client and server ports must be different.\n"
        );
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int argument_is_ledger_summary_command(
    const char *argument_text
) {
    LOG_TRACE("argument_is_ledger_summary_command(): now we check whether the user asked for ledger totals");

    return strcmp(
        argument_text,
        LEDGER_SUMMARY_ARGUMENT_TEXT
    ) == 0;
}

static int argument_is_identifier_command(
    const char *argument_text
) {
    LOG_TRACE("argument_is_identifier_command(): now we check whether the user asked for the local id");

    return strcmp(
        argument_text,
        IDENTIFIER_ARGUMENT_TEXT
    ) == 0;
}

static int argument_is_help_command(
    const char *argument_text
) {
    LOG_TRACE("argument_is_help_command(): now we check whether the user asked to see command help");

    return strcmp(
        argument_text,
        HELP_ARGUMENT_TEXT
    ) == 0;
}

static int argument_is_home_command(
    const char *argument_text
) {
    LOG_TRACE("argument_is_home_command(): now we check whether the user asked for the app storage folder");

    return strcmp(
        argument_text,
        HOME_ARGUMENT_TEXT
    ) == 0;
}

static void initialize_program_arguments(
    struct program_arguments *program_arguments
) {
    LOG_TRACE("initialize_program_arguments(): now we set defaults before applying user arguments");

    program_arguments->client_port = DEFAULT_CLIENT_PORT;
    program_arguments->server_port = DEFAULT_SERVER_PORT;
    program_arguments->app_storage_directory_path = NULL;
    program_arguments->program_mode = PROGRAM_MODE_RUN_SERVER;
}

static int parse_ledger_summary_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_ledger_summary_arguments(): now we parse the command that prints ledger owned and owed totals");

    if (argument_count != LEDGER_SUMMARY_ARGUMENT_COUNT
        && argument_count != LEDGER_SUMMARY_WITH_STORAGE_ARGUMENT_COUNT
    ) {
        LOG_WARN("Ledger summary arguments are unwanted because the command accepts only an optional storage directory");
        return TALKSPHERE_FAILURE;
    }

    program_arguments->program_mode = PROGRAM_MODE_PRINT_LEDGER_SUMMARY;

    if (argument_count == LEDGER_SUMMARY_WITH_STORAGE_ARGUMENT_COUNT) {
        program_arguments->app_storage_directory_path =
            argument_values[LEDGER_SUMMARY_STORAGE_DIRECTORY_ARGUMENT_INDEX];
    }

    return TALKSPHERE_SUCCESS;
}

static int parse_identifier_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_identifier_arguments(): now we parse the command that prints the local id");

    if (argument_count != IDENTIFIER_ARGUMENT_COUNT
        && argument_count != IDENTIFIER_WITH_STORAGE_ARGUMENT_COUNT
    ) {
        LOG_WARN("Identifier arguments are unwanted because the command accepts only an optional storage directory");
        return TALKSPHERE_FAILURE;
    }

    program_arguments->program_mode = PROGRAM_MODE_PRINT_IDENTIFIER;

    if (argument_count == IDENTIFIER_WITH_STORAGE_ARGUMENT_COUNT) {
        program_arguments->app_storage_directory_path =
            argument_values[IDENTIFIER_STORAGE_DIRECTORY_ARGUMENT_INDEX];
    }

    return TALKSPHERE_SUCCESS;
}

static int parse_help_arguments(
    int argument_count,
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_help_arguments(): now we parse the command that prints all supported arguments");

    if (argument_count != HELP_ARGUMENT_COUNT) {
        LOG_WARN("Help arguments are unwanted because --help does not accept extra arguments");
        return TALKSPHERE_FAILURE;
    }

    program_arguments->program_mode = PROGRAM_MODE_PRINT_HELP;
    return TALKSPHERE_SUCCESS;
}

static int parse_home_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_home_arguments(): now we parse the command that prints the app storage folder");

    if (argument_count != HOME_ARGUMENT_COUNT
        && argument_count != HOME_WITH_STORAGE_ARGUMENT_COUNT
    ) {
        LOG_WARN("Home arguments are unwanted because the command accepts only an optional storage directory");
        return TALKSPHERE_FAILURE;
    }

    program_arguments->program_mode = PROGRAM_MODE_PRINT_HOME;

    if (argument_count == HOME_WITH_STORAGE_ARGUMENT_COUNT) {
        program_arguments->app_storage_directory_path =
            argument_values[HOME_STORAGE_DIRECTORY_ARGUMENT_INDEX];
    }

    return TALKSPHERE_SUCCESS;
}

static int parse_server_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_server_arguments(): now we parse arguments used to run the socket server");

    if (argument_count != DEFAULT_ARGUMENT_COUNT
        && argument_count != CUSTOM_PORT_ARGUMENT_COUNT
        && argument_count != CUSTOM_PORT_AND_STORAGE_ARGUMENT_COUNT
    ) {
        LOG_WARN("Argument count is unwanted because the program accepts no arguments, both ports, or both ports with storage");
        return TALKSPHERE_FAILURE;
    }

    if (argument_count == CUSTOM_PORT_ARGUMENT_COUNT
        || argument_count == CUSTOM_PORT_AND_STORAGE_ARGUMENT_COUNT
    ) {
        LOG_TRACE("parse_server_arguments(): custom ports were provided so we parse both explicitly");

        if (parse_port(
                argument_values[CLIENT_PORT_ARGUMENT_INDEX],
                "client",
                &program_arguments->client_port
            ) != TALKSPHERE_SUCCESS
            || parse_port(
                argument_values[SERVER_PORT_ARGUMENT_INDEX],
                "server",
                &program_arguments->server_port
            ) != TALKSPHERE_SUCCESS
        ) {
            return TALKSPHERE_FAILURE;
        }

        if (argument_count == CUSTOM_PORT_AND_STORAGE_ARGUMENT_COUNT) {
            program_arguments->app_storage_directory_path = argument_values[STORAGE_DIRECTORY_ARGUMENT_INDEX];
        }
    }

    return validate_different_ports(program_arguments);
}

int parse_program_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_program_arguments(): now we turn process arguments into socket or ledger settings");
    LOG_DEBUG(
        "Received %d program arguments",
        argument_count
    );

    const char *program_name = argument_values[PROGRAM_NAME_ARGUMENT_INDEX];
    initialize_program_arguments(program_arguments);

    int parse_result = TALKSPHERE_FAILURE;
    if (argument_count >= LEDGER_SUMMARY_ARGUMENT_COUNT
        && argument_is_ledger_summary_command(argument_values[LEDGER_SUMMARY_ARGUMENT_INDEX])
    ) {
        parse_result = parse_ledger_summary_arguments(
            argument_count,
            argument_values,
            program_arguments
        );
    } else if (argument_count >= IDENTIFIER_ARGUMENT_COUNT
        && argument_is_identifier_command(argument_values[IDENTIFIER_ARGUMENT_INDEX])
    ) {
        parse_result = parse_identifier_arguments(
            argument_count,
            argument_values,
            program_arguments
        );
    } else if (argument_count >= HOME_ARGUMENT_COUNT
        && argument_is_home_command(argument_values[HOME_ARGUMENT_INDEX])
    ) {
        parse_result = parse_home_arguments(
            argument_count,
            argument_values,
            program_arguments
        );
    } else if (argument_count >= HELP_ARGUMENT_COUNT
        && argument_is_help_command(argument_values[HELP_ARGUMENT_INDEX])
    ) {
        parse_result = parse_help_arguments(
            argument_count,
            program_arguments
        );
    } else {
        parse_result = parse_server_arguments(
            argument_count,
            argument_values,
            program_arguments
        );
    }

    if (parse_result != TALKSPHERE_SUCCESS) {
        print_usage(
            stderr,
            program_name
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_PRINT_HELP) {
        print_usage(
            stdout,
            program_name
        );
    }

    return parse_result;
}
