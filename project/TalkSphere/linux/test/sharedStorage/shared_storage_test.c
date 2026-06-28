#include "sharedStorage/shared_storage.h"
#include "common/result.h"
#include "test_support.h"

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define STORE_AND_RECOVER_STORE_FAILED 10
#define STORE_AND_RECOVER_RECOVER_FAILED 11
#define STORE_AND_RECOVER_BYTE_COUNT_MISMATCH 12
#define STORE_AND_RECOVER_CONTENT_MISMATCH 13
#define WRONG_OWNER_RECOVERY_SUCCEEDED 20
#define QUERY_FILE_MANAGER_QUERY_FAILED 30
#define QUERY_FILE_MANAGER_COUNT_MISMATCH 31
#define QUERY_FILE_MANAGER_MUTATING_SQL_ACCEPTED 32
#define FORCE_DELETE_DELETE_FAILED 40
#define FORCE_DELETE_RECOVERY_SUCCEEDED 41
#define EXPIRED_CLEANUP_STORE_EXPIRED_FAILED 50
#define EXPIRED_CLEANUP_STORE_ACTIVE_FAILED 51
#define EXPIRED_CLEANUP_CLEANUP_FAILED 52
#define EXPIRED_CLEANUP_EXPIRED_RECOVERY_SUCCEEDED 53
#define EXPIRED_CLEANUP_ACTIVE_RECOVERY_FAILED 54
#define INVALID_INPUT_NULL_BYTES_ACCEPTED 60
#define INVALID_INPUT_EMPTY_FILE_ID_ACCEPTED 61
#define APP_STORAGE_PATH_TOO_LONG 70
#define PLACEHOLDER_SHARE_AVAILABLE_STORAGE_FAILED 71
#define PLACEHOLDER_RECOVER_SOLD_STORAGE_FAILED 72
#define PLACEHOLDER_CLEAR_AGED_STORAGE_FAILED 73
#define SINGLE_STORED_FILE_COUNT 1
#define WRONG_OWNER_RECOVERY_BUFFER_SIZE 16
#define CLEANUP_RECOVERY_BUFFER_SIZE 16
#define FORCE_DELETE_RECOVERY_BUFFER_SIZE 16
#define PLACEHOLDER_MAXIMUM_STORAGE_AGE_SECONDS 60
#define EXPIRED_FILE_EXPIRATION_SECONDS 50
#define CLEANUP_CURRENT_TIME_SECONDS 100
#define STORE_AND_RECOVER_EXPIRATION_SECONDS 200
#define ACTIVE_FILE_EXPIRATION_SECONDS 500
#define INVALID_INPUT_EXPIRATION_SECONDS 1
#define APP_STORAGE_DIRECTORY_PATH_SIZE 256
#define NO_QUERY_COLUMNS 0
#define STORED_FILE_COUNT_COLUMN_INDEX 0

struct query_count_context {
    int row_count;
    int stored_file_count;
};

static char *test_app_storage_directory_path;

static int count_query_rows(
    int column_count,
    const char **column_names,
    const char **column_values,
    void *callback_context
) {
    (void)column_names;

    struct query_count_context *query_count_context = callback_context;
    query_count_context->row_count++;

    if (column_count > NO_QUERY_COLUMNS && column_values[STORED_FILE_COUNT_COLUMN_INDEX] != NULL) {
        query_count_context->stored_file_count = atoi(column_values[STORED_FILE_COUNT_COLUMN_INDEX]);
    }

    return TALKSPHERE_SUCCESS;
}

static int test_store_and_recover_data(
    const char *app_storage_directory_path
) {
    const unsigned char stored_file_bytes[] = {
        1,
        2,
        3,
        4,
        9
    };

    if (shared_storage_store_data(
            app_storage_directory_path,
            stored_file_bytes,
            sizeof(stored_file_bytes),
            "file-one",
            "owner-one",
            STORE_AND_RECOVER_EXPIRATION_SECONDS
        ) != TALKSPHERE_SUCCESS
    ) {
        return STORE_AND_RECOVER_STORE_FAILED;
    }

    unsigned char recovered_file_bytes[sizeof(stored_file_bytes)];
    size_t recovered_file_byte_count = 0;
    if (shared_storage_recover_data(
            app_storage_directory_path,
            "file-one",
            "owner-one",
            recovered_file_bytes,
            sizeof(recovered_file_bytes),
            &recovered_file_byte_count
        ) != TALKSPHERE_SUCCESS
    ) {
        return STORE_AND_RECOVER_RECOVER_FAILED;
    }

    if (recovered_file_byte_count != sizeof(stored_file_bytes)) {
        return STORE_AND_RECOVER_BYTE_COUNT_MISMATCH;
    }

    if (memcmp(
            stored_file_bytes,
            recovered_file_bytes,
            sizeof(stored_file_bytes)
        ) != 0
    ) {
        return STORE_AND_RECOVER_CONTENT_MISMATCH;
    }

    return TEST_SUCCESS;
}

static int test_recover_rejects_wrong_owner(
    const char *app_storage_directory_path
) {
    unsigned char recovered_file_bytes[WRONG_OWNER_RECOVERY_BUFFER_SIZE];
    size_t recovered_file_byte_count = 0;

    if (shared_storage_recover_data(
            app_storage_directory_path,
            "file-one",
            "different-owner",
            recovered_file_bytes,
            sizeof(recovered_file_bytes),
            &recovered_file_byte_count
        ) != TALKSPHERE_FAILURE
    ) {
        return WRONG_OWNER_RECOVERY_SUCCEEDED;
    }

    return TEST_SUCCESS;
}

static int test_query_file_manager_data(
    const char *app_storage_directory_path
) {
    struct query_count_context query_count_context = {
        .row_count = 0,
        .stored_file_count = 0
    };

    if (shared_storage_query_file_manager(
            app_storage_directory_path,
            "SELECT COUNT(*) FROM shared_files WHERE shared_file_id = 'file-one' AND owner_id = 'owner-one'",
            count_query_rows,
            &query_count_context
        ) != TALKSPHERE_SUCCESS
    ) {
        return QUERY_FILE_MANAGER_QUERY_FAILED;
    }

    if (
        query_count_context.row_count != SINGLE_STORED_FILE_COUNT
        || query_count_context.stored_file_count != SINGLE_STORED_FILE_COUNT
    ) {
        return QUERY_FILE_MANAGER_COUNT_MISMATCH;
    }

    if (shared_storage_query_file_manager(
            app_storage_directory_path,
            "DELETE FROM shared_files",
            count_query_rows,
            &query_count_context
        ) != TALKSPHERE_FAILURE
    ) {
        return QUERY_FILE_MANAGER_MUTATING_SQL_ACCEPTED;
    }

    return TEST_SUCCESS;
}

static int test_force_delete_entry(
    const char *app_storage_directory_path
) {
    if (shared_storage_delete_entry(
            app_storage_directory_path,
            "file-one",
            "owner-one"
        ) != TALKSPHERE_SUCCESS
    ) {
        return FORCE_DELETE_DELETE_FAILED;
    }

    unsigned char recovered_file_bytes[FORCE_DELETE_RECOVERY_BUFFER_SIZE];
    size_t recovered_file_byte_count = 0;
    if (shared_storage_recover_data(
            app_storage_directory_path,
            "file-one",
            "owner-one",
            recovered_file_bytes,
            sizeof(recovered_file_bytes),
            &recovered_file_byte_count
        ) != TALKSPHERE_FAILURE
    ) {
        return FORCE_DELETE_RECOVERY_SUCCEEDED;
    }

    return TEST_SUCCESS;
}

static int test_cleanup_expired_entries(
    const char *app_storage_directory_path
) {
    const unsigned char expired_file_bytes[] = {
        8,
        5,
        3
    };
    const unsigned char active_file_bytes[] = {
        13,
        21
    };

    if (shared_storage_store_data(
            app_storage_directory_path,
            expired_file_bytes,
            sizeof(expired_file_bytes),
            "expired-file",
            "owner-two",
            EXPIRED_FILE_EXPIRATION_SECONDS
        ) != TALKSPHERE_SUCCESS
    ) {
        return EXPIRED_CLEANUP_STORE_EXPIRED_FAILED;
    }

    if (shared_storage_store_data(
            app_storage_directory_path,
            active_file_bytes,
            sizeof(active_file_bytes),
            "active-file",
            "owner-two",
            ACTIVE_FILE_EXPIRATION_SECONDS
        ) != TALKSPHERE_SUCCESS
    ) {
        return EXPIRED_CLEANUP_STORE_ACTIVE_FAILED;
    }

    if (shared_storage_clean_up_expired_entries(
            app_storage_directory_path,
            CLEANUP_CURRENT_TIME_SECONDS
        ) != TALKSPHERE_SUCCESS
    ) {
        return EXPIRED_CLEANUP_CLEANUP_FAILED;
    }

    unsigned char recovered_file_bytes[CLEANUP_RECOVERY_BUFFER_SIZE];
    size_t recovered_file_byte_count = 0;
    if (shared_storage_recover_data(
            app_storage_directory_path,
            "expired-file",
            "owner-two",
            recovered_file_bytes,
            sizeof(recovered_file_bytes),
            &recovered_file_byte_count
        ) != TALKSPHERE_FAILURE
    ) {
        return EXPIRED_CLEANUP_EXPIRED_RECOVERY_SUCCEEDED;
    }

    if (shared_storage_recover_data(
            app_storage_directory_path,
            "active-file",
            "owner-two",
            recovered_file_bytes,
            sizeof(recovered_file_bytes),
            &recovered_file_byte_count
        ) != TALKSPHERE_SUCCESS
    ) {
        return EXPIRED_CLEANUP_ACTIVE_RECOVERY_FAILED;
    }

    return TEST_SUCCESS;
}

static int test_invalid_inputs(
    const char *app_storage_directory_path
) {
    const unsigned char stored_file_bytes[] = {
        1
    };

    if (shared_storage_store_data(
            app_storage_directory_path,
            NULL,
            sizeof(stored_file_bytes),
            "file",
            "owner",
            INVALID_INPUT_EXPIRATION_SECONDS
        ) != TALKSPHERE_FAILURE
    ) {
        return INVALID_INPUT_NULL_BYTES_ACCEPTED;
    }

    if (shared_storage_store_data(
            app_storage_directory_path,
            stored_file_bytes,
            sizeof(stored_file_bytes),
            "",
            "owner",
            INVALID_INPUT_EXPIRATION_SECONDS
        ) != TALKSPHERE_FAILURE
    ) {
        return INVALID_INPUT_EMPTY_FILE_ID_ACCEPTED;
    }

    return TEST_SUCCESS;
}

static int test_prepare_shared_storage_path(void) {
    if (snprintf(
            test_app_storage_directory_path,
            APP_STORAGE_DIRECTORY_PATH_SIZE,
            "/tmp/talksphere-shared-storage-test-%ld",
            (long)getpid()
        ) >= APP_STORAGE_DIRECTORY_PATH_SIZE
    ) {
        return APP_STORAGE_PATH_TOO_LONG;
    }

    return TEST_SUCCESS;
}

static int test_store_and_recover_data_case(void) {
    return test_store_and_recover_data(test_app_storage_directory_path);
}

static int test_recover_rejects_wrong_owner_case(void) {
    return test_recover_rejects_wrong_owner(test_app_storage_directory_path);
}

static int test_query_file_manager_data_case(void) {
    return test_query_file_manager_data(test_app_storage_directory_path);
}

static int test_force_delete_entry_case(void) {
    return test_force_delete_entry(test_app_storage_directory_path);
}

static int test_cleanup_expired_entries_case(void) {
    return test_cleanup_expired_entries(test_app_storage_directory_path);
}

static int test_invalid_inputs_case(void) {
    return test_invalid_inputs(test_app_storage_directory_path);
}

static int test_placeholder_operations(void) {
    if (shared_storage_share_available_storage(test_app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return PLACEHOLDER_SHARE_AVAILABLE_STORAGE_FAILED;
    }

    if (shared_storage_recover_sold_storage(test_app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return PLACEHOLDER_RECOVER_SOLD_STORAGE_FAILED;
    }

    if (shared_storage_clear_aged_storage(
            test_app_storage_directory_path,
            PLACEHOLDER_MAXIMUM_STORAGE_AGE_SECONDS
        ) != TALKSPHERE_SUCCESS
    ) {
        return PLACEHOLDER_CLEAR_AGED_STORAGE_FAILED;
    }

    return TEST_SUCCESS;
}

int main(void) {
    char app_storage_directory_path[APP_STORAGE_DIRECTORY_PATH_SIZE];
    test_app_storage_directory_path = app_storage_directory_path;

    const struct test_case test_cases[] = {
        TEST_CASE(test_prepare_shared_storage_path),
        TEST_CASE(test_store_and_recover_data_case),
        TEST_CASE(test_recover_rejects_wrong_owner_case),
        TEST_CASE(test_query_file_manager_data_case),
        TEST_CASE(test_force_delete_entry_case),
        TEST_CASE(test_cleanup_expired_entries_case),
        TEST_CASE(test_invalid_inputs_case),
        TEST_CASE(test_placeholder_operations)
    };

    return run_test_cases(
        test_cases,
        sizeof(test_cases) / sizeof(test_cases[0])
    );
}
