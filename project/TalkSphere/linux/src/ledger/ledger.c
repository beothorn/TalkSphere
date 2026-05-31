#include "ledger.h"

#include "../common/result.h"
#include "../logging.h"

#include <limits.h>
#include <stdio.h>

#define LEDGER_DIRECTORY_NAME "ledger"

static int build_ledger_file_path(
    const char *app_storage_directory_path,
    const char *identifier_text,
    char *ledger_file_path,
    size_t ledger_file_path_size
) {
    LOG_TRACE("build_ledger_file_path(): now we build where one id has its credit counter");

    if (snprintf(
            ledger_file_path,
            ledger_file_path_size,
            "%s/%s/%s",
            app_storage_directory_path,
            LEDGER_DIRECTORY_NAME,
            identifier_text
        ) >= (int)ledger_file_path_size
    ) {
        LOG_ERROR("Ledger file path is too long so ledger operation cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int ledger_get_credits(
    const char *app_storage_directory_path,
    const char *identifier_text,
    int *credit_count
) {
    LOG_TRACE("ledger_get_credits(): now we read how many credits this id has");

    char ledger_file_path[PATH_MAX];
    if (build_ledger_file_path(
            app_storage_directory_path,
            identifier_text,
            ledger_file_path,
            sizeof(ledger_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    FILE *ledger_file = fopen(ledger_file_path, "r");

    if (ledger_file == NULL) {
        *credit_count = 0;
        return TALKSPHERE_SUCCESS;
    }

    if (fscanf(ledger_file, "%d", credit_count) != 1) {
        fclose(ledger_file);
        LOG_WARN("Ledger file format is unwanted because it does not contain a valid integer credit count");
        return TALKSPHERE_FAILURE;
    }

    fclose(ledger_file);
    return TALKSPHERE_SUCCESS;
}

static int write_credits(
    const char *app_storage_directory_path,
    const char *identifier_text,
    int credit_count
) {
    LOG_TRACE("write_credits(): now we persist an updated credit value for one id");

    char ledger_file_path[PATH_MAX];
    if (build_ledger_file_path(
            app_storage_directory_path,
            identifier_text,
            ledger_file_path,
            sizeof(ledger_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    FILE *ledger_file = fopen(ledger_file_path, "w");

    if (ledger_file == NULL) {
        LOG_ERROR("Opening ledger file for write failed so credit update cannot continue");
        return TALKSPHERE_FAILURE;
    }

    if (fprintf(ledger_file, "%d", credit_count) < 0) {
        fclose(ledger_file);
        LOG_ERROR("Writing ledger file failed so credit update cannot continue");
        return TALKSPHERE_FAILURE;
    }

    fclose(ledger_file);
    return TALKSPHERE_SUCCESS;
}

int ledger_add_credit(
    const char *app_storage_directory_path,
    const char *identifier_text
) {
    LOG_TRACE("ledger_add_credit(): now we increase credits for one id by one");

    int credit_count = 0;
    if (ledger_get_credits(
            app_storage_directory_path,
            identifier_text,
            &credit_count
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return write_credits(
        app_storage_directory_path,
        identifier_text,
        credit_count + 1
    );
}

int ledger_spend_credit(
    const char *app_storage_directory_path,
    const char *identifier_text
) {
    LOG_TRACE("ledger_spend_credit(): now we decrease credits for one id when a message is accepted");

    int credit_count = 0;
    if (ledger_get_credits(
            app_storage_directory_path,
            identifier_text,
            &credit_count
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (credit_count <= 0) {
        LOG_WARN("Credit spend is unwanted because this id does not have enough credits");
        return TALKSPHERE_FAILURE;
    }

    return write_credits(
        app_storage_directory_path,
        identifier_text,
        credit_count - 1
    );
}
