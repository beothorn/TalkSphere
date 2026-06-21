#include "credit_withdraw_application.h"

#include "credit_withdraw.h"

#include "../common/result.h"
#include "../files/app_files.h"
#include "../logging.h"

#include <stdio.h>

#define IDENTIFIER_TEXT_SIZE 256

int credit_withdraw_application_add_code(
    const char *resolved_storage_directory_path,
    int credit_count,
    const char *credit_code_text
) {
    LOG_TRACE("credit_withdraw_application_add_code(): now we store a credit withdraw code for the local id");

    char local_identifier_text[IDENTIFIER_TEXT_SIZE];
    if (read_local_identifier(
            resolved_storage_directory_path,
            local_identifier_text,
            sizeof(local_identifier_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (credit_withdraw_add_code(
            resolved_storage_directory_path,
            local_identifier_text,
            credit_count,
            credit_code_text
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Stored %d credit for code %s\n",
        credit_count,
        credit_code_text
    );
    return TALKSPHERE_SUCCESS;
}

int credit_withdraw_application_remove_code(
    const char *resolved_storage_directory_path,
    const char *credit_code_text
) {
    LOG_TRACE("credit_withdraw_application_remove_code(): now we remove a credit withdraw code from local storage");

    if (credit_withdraw_remove_code(
            resolved_storage_directory_path,
            credit_code_text
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Removed credit code %s\n",
        credit_code_text
    );
    return TALKSPHERE_SUCCESS;
}

static int print_credit_withdraw_entry(
    const struct credit_withdraw_entry *credit_withdraw_entry,
    void *callback_context
) {
    LOG_TRACE("print_credit_withdraw_entry(): now we print one credit withdraw row for the list command");

    (void)callback_context;
    printf(
        "%d %s\n",
        credit_withdraw_entry->credit_count,
        credit_withdraw_entry->credit_code_text
    );
    return TALKSPHERE_SUCCESS;
}

int credit_withdraw_application_list_codes(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("credit_withdraw_application_list_codes(): now we print credits and ids from stored credit withdraw codes");

    return credit_withdraw_list_codes(
        resolved_storage_directory_path,
        print_credit_withdraw_entry,
        NULL
    );
}

int credit_withdraw_application_print_add_dry_run(
    const char *resolved_storage_directory_path,
    int credit_count,
    const char *credit_code_text
) {
    LOG_TRACE("credit_withdraw_application_print_add_dry_run(): now we describe credit withdraw code storage without changing state");

    printf(
        "Would add %d credit withdraw code %s in %s\n",
        credit_count,
        credit_code_text,
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}

int credit_withdraw_application_print_remove_dry_run(
    const char *resolved_storage_directory_path,
    const char *credit_code_text
) {
    LOG_TRACE("credit_withdraw_application_print_remove_dry_run(): now we describe credit withdraw code removal without changing state");

    printf(
        "Would remove credit withdraw code %s from %s\n",
        credit_code_text,
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}

int credit_withdraw_application_print_list_dry_run(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("credit_withdraw_application_print_list_dry_run(): now we describe credit withdraw code listing without changing state");

    printf(
        "Would list credit withdraw codes from %s\n",
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}
