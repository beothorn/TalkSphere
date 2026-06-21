#include "files_application.h"

#include "../common/result.h"
#include "../logging.h"

#include <stdio.h>

int files_application_print_home(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("files_application_print_home(): now we print the resolved storage home folder");

    printf(
        "%s\n",
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}

int files_application_print_home_dry_run(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("files_application_print_home_dry_run(): now we describe the home folder command without changing state");

    printf(
        "Would print home folder %s\n",
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}
