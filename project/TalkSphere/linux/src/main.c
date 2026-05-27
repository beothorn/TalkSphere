#include "logging.h"
#include "argumentParsing/program_arguments.h"
#include "network/socket_channel.h"

/*
 * This is intentionally thin: main only wires the entrypoint steps together.
 */
int main(
    int argument_count,
    char *argument_values[]
) {
    LOG_TRACE("main(): starting the program entrypoint");

    struct program_arguments program_arguments;

    if (parse_program_arguments(
            argument_count,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return run_socket_channel(&program_arguments);
}