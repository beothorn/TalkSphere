#ifndef TALKSPHERE_PROGRAM_ARGUMENTS_H
#define TALKSPHERE_PROGRAM_ARGUMENTS_H

#include "../common/result.h"

#define DEFAULT_SERVER_PORT 8513
#define DEFAULT_CLIENT_PORT 8512

struct program_arguments {
    int client_port;
    int server_port;
};

int parse_program_arguments(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
);

#endif
