#include "shared_storage_application.h"

#include "../common/result.h"
#include "../logging.h"

#include <stdio.h>

static int print_placeholder(
    const char *placeholder_text
) {
    LOG_TRACE("print_placeholder(): now shared storage reports a command that exists before the feature body is implemented");

    printf(
        "%s\n",
        placeholder_text
    );
    return TALKSPHERE_SUCCESS;
}

int shared_storage_application_print_local_list_dry_run(void) {
    LOG_TRACE("shared_storage_application_print_local_list_dry_run(): now we describe local shared storage listing without changing state");

    printf("Would list local shared storage metadata\n");
    return TALKSPHERE_SUCCESS;
}

int shared_storage_application_print_remote_list_dry_run(void) {
    LOG_TRACE("shared_storage_application_print_remote_list_dry_run(): now we describe remote shared storage listing without changing state");

    printf("Would list remote shared storage metadata\n");
    return TALKSPHERE_SUCCESS;
}

int shared_storage_application_print_local_list_placeholder(void) {
    LOG_TRACE("shared_storage_application_print_local_list_placeholder(): now we report that local shared storage listing is not implemented yet");

    return print_placeholder("local shared storage listing is not implemented yet");
}

int shared_storage_application_print_remote_list_placeholder(void) {
    LOG_TRACE("shared_storage_application_print_remote_list_placeholder(): now we report that remote shared storage listing is not implemented yet");

    return print_placeholder("remote shared storage listing is not implemented yet");
}
