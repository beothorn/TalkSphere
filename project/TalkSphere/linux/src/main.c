#include "logging.h"
#include "files/app_files.h"
#include "network/socket_channel.h"

int main(
    int argument_count,
    char *argument_values[]
) {
    (void)argument_count;
    (void)argument_values;

    LOG_TRACE("main(): starting the program entrypoint");

    if (ensure_app_files() != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return run_socket_channel();
}
