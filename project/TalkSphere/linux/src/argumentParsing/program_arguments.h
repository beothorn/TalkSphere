#ifndef TALKSPHERE_PROGRAM_ARGUMENTS_H
#define TALKSPHERE_PROGRAM_ARGUMENTS_H

#include "../common/result.h"

#define DEFAULT_SERVER_PORT 8513
#define DEFAULT_CLIENT_PORT 8512

enum program_mode {
    PROGRAM_MODE_RUN_SERVER,
    PROGRAM_MODE_PRINT_LEDGER_SUMMARY,
    PROGRAM_MODE_PRINT_IDENTIFIER,
    PROGRAM_MODE_PRINT_HOME,
    PROGRAM_MODE_PRINT_HELP
};

struct program_arguments {
    int client_port;
    int server_port;
    const char *app_storage_directory_path;
    enum program_mode program_mode;
};

int parse_program_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
);

#endif
