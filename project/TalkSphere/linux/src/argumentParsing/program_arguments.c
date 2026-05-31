#include "program_arguments.h"

#include "logging.h"

#include <stdio.h>
#include <stdlib.h>

#define PROGRAM_NAME_ARGUMENT_INDEX 0
#define FIRST_CHARACTER_INDEX 0
#define CLIENT_PORT_ARGUMENT_INDEX 1
#define SERVER_PORT_ARGUMENT_INDEX 2
#define DEFAULT_ARGUMENT_COUNT 1
#define CUSTOM_PORT_ARGUMENT_COUNT 3
#define CUSTOM_PORT_AND_STORAGE_ARGUMENT_COUNT 4
#define STORAGE_DIRECTORY_ARGUMENT_INDEX 3

#define DECIMAL_BASE 10
#define MINIMUM_PORT 1
#define MAXIMUM_PORT 65535
#define STRING_TERMINATOR '\0'

static void print_usage(
    const char *program_name
) {
    LOG_TRACE("print_usage(): now we print the valid command shapes");

    fprintf(
        stderr,
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

int parse_program_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
) {
    LOG_TRACE("parse_program_arguments(): now we turn process arguments into socket settings");
    LOG_DEBUG(
        "Received %d program arguments",
        argument_count
    );

    const char *program_name = argument_values[PROGRAM_NAME_ARGUMENT_INDEX];

    program_arguments->client_port = DEFAULT_CLIENT_PORT;
    program_arguments->server_port = DEFAULT_SERVER_PORT;
    program_arguments->app_storage_directory_path = NULL;

    if (argument_count != DEFAULT_ARGUMENT_COUNT
        && argument_count != CUSTOM_PORT_ARGUMENT_COUNT
        && argument_count != CUSTOM_PORT_AND_STORAGE_ARGUMENT_COUNT
    ) {
        LOG_WARN("Argument count is unwanted because the program accepts either no ports or both ports");
        print_usage(program_name);
        return TALKSPHERE_FAILURE;
    }

    if (argument_count == CUSTOM_PORT_ARGUMENT_COUNT
        || argument_count == CUSTOM_PORT_AND_STORAGE_ARGUMENT_COUNT
    ) {
        LOG_TRACE("parse_program_arguments(): custom ports were provided so we parse both explicitly");

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
            print_usage(program_name);
            return TALKSPHERE_FAILURE;
        }

        if (argument_count == CUSTOM_PORT_AND_STORAGE_ARGUMENT_COUNT) {
            program_arguments->app_storage_directory_path = argument_values[STORAGE_DIRECTORY_ARGUMENT_INDEX];
        }
    }

    return validate_different_ports(program_arguments);
}
