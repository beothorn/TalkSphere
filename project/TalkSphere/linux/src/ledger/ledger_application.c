#include "ledger_application.h"

#include "ledger_summary.h"

#include "../common/result.h"
#include "../files/app_files.h"
#include "../logging.h"

#include <stdio.h>

#define IDENTIFIER_TEXT_SIZE 256

int ledger_application_print_local_summary(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("ledger_application_print_local_summary(): now we read the local id before asking ledger to print the summary");

    char local_identifier_text[IDENTIFIER_TEXT_SIZE];
    if (read_local_identifier(
            resolved_storage_directory_path,
            local_identifier_text,
            sizeof(local_identifier_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return print_ledger_summary(
        resolved_storage_directory_path,
        local_identifier_text
    );
}

int ledger_application_print_summary_dry_run(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("ledger_application_print_summary_dry_run(): now we describe ledger summary printing without changing state");

    printf(
        "Would print ledger credit summary from %s\n",
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}
