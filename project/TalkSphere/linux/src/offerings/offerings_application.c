#include "offerings_application.h"

#include "offerings.h"

#include "../common/result.h"
#include "../logging.h"

#include <stdio.h>

#define OFFERINGS_TEXT_SIZE 8192

static int print_placeholder(
    const char *placeholder_text
) {
    LOG_TRACE("print_placeholder(): now offerings reports a command that exists before the feature body is implemented");

    printf(
        "%s\n",
        placeholder_text
    );
    return TALKSPHERE_SUCCESS;
}

int offerings_application_print_local(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("offerings_application_print_local(): now we print the local offerings document");

    char offerings_text[OFFERINGS_TEXT_SIZE];
    if (read_local_offerings(
            resolved_storage_directory_path,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "%s",
        offerings_text
    );
    return TALKSPHERE_SUCCESS;
}

int offerings_application_print_local_dry_run(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("offerings_application_print_local_dry_run(): now we describe local offerings printing without changing state");

    printf(
        "Would print local offerings from %s\n",
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}

int offerings_application_print_add_dry_run(
    const char *offering_text
) {
    LOG_TRACE("offerings_application_print_add_dry_run(): now we describe adding an offering without changing state");

    printf(
        "Would add offering: %s\n",
        offering_text
    );
    return TALKSPHERE_SUCCESS;
}

int offerings_application_print_edit_dry_run(
    const char *offering_text
) {
    LOG_TRACE("offerings_application_print_edit_dry_run(): now we describe editing an offering without changing state");

    printf(
        "Would edit offering: %s\n",
        offering_text
    );
    return TALKSPHERE_SUCCESS;
}

int offerings_application_print_remove_dry_run(
    const char *offering_text
) {
    LOG_TRACE("offerings_application_print_remove_dry_run(): now we describe removing an offering without changing state");

    printf(
        "Would remove offering: %s\n",
        offering_text
    );
    return TALKSPHERE_SUCCESS;
}

int offerings_application_print_add_placeholder(void) {
    LOG_TRACE("offerings_application_print_add_placeholder(): now we report that offering add is not implemented yet");

    return print_placeholder("offering add is not implemented yet");
}

int offerings_application_print_edit_placeholder(void) {
    LOG_TRACE("offerings_application_print_edit_placeholder(): now we report that offering edit is not implemented yet");

    return print_placeholder("offering edit is not implemented yet");
}

int offerings_application_print_remove_placeholder(void) {
    LOG_TRACE("offerings_application_print_remove_placeholder(): now we report that offering remove is not implemented yet");

    return print_placeholder("offering remove is not implemented yet");
}
