#include "logging.h"
#include "program_arguments.h"
#include "socket_basics.h"

/*
 * This is intentionally thin: main only wires the entrypoint steps together.
 */
int main(
    int argument_count,
    char *argument_values[]
) {
    TALKSPHERE_LOG_TRACE("main(): starting the program entrypoint");

    struct program_arguments program_arguments;

    if (parse_program_arguments(
            argument_count,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return run_socket_basics(&program_arguments);
}
