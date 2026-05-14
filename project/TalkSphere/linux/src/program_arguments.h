#ifndef TALKSPHERE_PROGRAM_ARGUMENTS_H
#define TALKSPHERE_PROGRAM_ARGUMENTS_H

#define TALKSPHERE_SUCCESS 0
#define TALKSPHERE_FAILURE 1

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
