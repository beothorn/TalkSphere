#include "application/application.h"
#include "logging.h"

int main(
    int argument_count,
    char *argument_values[]
) {
    LOG_TRACE("main(): starting the program entrypoint");

    return run_talksphere_application(
        argument_count,
        argument_values
    );
}
