#include "creditWithdraw/credit_withdraw.h"
#include "common/result.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define TEST_SUCCESS 0
#define TEMP_DIRECTORY_TEMPLATE_SIZE 128
#define ADD_CODE_FAILED 10
#define FIND_CODE_FAILED 11
#define OWNER_IDENTIFIER_MISMATCH 12
#define CREDIT_COUNT_MISMATCH 13
#define REMOVE_CODE_FAILED 20
#define REMOVED_CODE_FOUND 21
#define MISSING_CODE_REMOVED 30
#define INVALID_CREDIT_COUNT_ACCEPTED 40
#define EXPECTED_CREDIT_COUNT 3
#define INVALID_CREDIT_COUNT 0
#define EXPECTED_OWNER_IDENTIFIER_TEXT "owner-one"
#define EXPECTED_CREDIT_CODE_TEXT "1234"
#define MISSING_CREDIT_CODE_TEXT "missing"

static int make_test_directory(
    char *test_directory_path,
    size_t test_directory_path_size
) {
    if (snprintf(
            test_directory_path,
            test_directory_path_size,
            "/tmp/talksphere-credit-withdraw-test-%ld",
            (long)getpid()
        ) >= (int)test_directory_path_size
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (mkdir(
            test_directory_path,
            0700
        ) != 0
    ) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int test_add_find_and_remove_code(
    const char *app_storage_directory_path
) {
    if (credit_withdraw_add_code(
            app_storage_directory_path,
            EXPECTED_OWNER_IDENTIFIER_TEXT,
            EXPECTED_CREDIT_COUNT,
            EXPECTED_CREDIT_CODE_TEXT
        ) != TALKSPHERE_SUCCESS
    ) {
        return ADD_CODE_FAILED;
    }

    struct credit_withdraw_entry credit_withdraw_entry;
    if (credit_withdraw_find_code(
            app_storage_directory_path,
            EXPECTED_CREDIT_CODE_TEXT,
            &credit_withdraw_entry
        ) != TALKSPHERE_SUCCESS
    ) {
        return FIND_CODE_FAILED;
    }

    if (strcmp(
            credit_withdraw_entry.owner_identifier_text,
            EXPECTED_OWNER_IDENTIFIER_TEXT
        ) != 0
    ) {
        return OWNER_IDENTIFIER_MISMATCH;
    }

    if (credit_withdraw_entry.credit_count != EXPECTED_CREDIT_COUNT) {
        return CREDIT_COUNT_MISMATCH;
    }

    if (credit_withdraw_remove_code(
            app_storage_directory_path,
            EXPECTED_CREDIT_CODE_TEXT
        ) != TALKSPHERE_SUCCESS
    ) {
        return REMOVE_CODE_FAILED;
    }

    if (credit_withdraw_find_code(
            app_storage_directory_path,
            EXPECTED_CREDIT_CODE_TEXT,
            &credit_withdraw_entry
        ) != TALKSPHERE_FAILURE
    ) {
        return REMOVED_CODE_FOUND;
    }

    return TEST_SUCCESS;
}

static int test_remove_rejects_missing_code(
    const char *app_storage_directory_path
) {
    if (credit_withdraw_remove_code(
            app_storage_directory_path,
            MISSING_CREDIT_CODE_TEXT
        ) != TALKSPHERE_FAILURE
    ) {
        return MISSING_CODE_REMOVED;
    }

    return TEST_SUCCESS;
}

static int test_add_rejects_invalid_credit_count(
    const char *app_storage_directory_path
) {
    if (credit_withdraw_add_code(
            app_storage_directory_path,
            EXPECTED_OWNER_IDENTIFIER_TEXT,
            INVALID_CREDIT_COUNT,
            EXPECTED_CREDIT_CODE_TEXT
        ) != TALKSPHERE_FAILURE
    ) {
        return INVALID_CREDIT_COUNT_ACCEPTED;
    }

    return TEST_SUCCESS;
}

int main(void) {
    char test_directory_path[TEMP_DIRECTORY_TEMPLATE_SIZE];
    if (make_test_directory(
            test_directory_path,
            sizeof(test_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return 1;
    }

    int test_result = test_add_find_and_remove_code(test_directory_path);
    if (test_result != TEST_SUCCESS) {
        return test_result;
    }

    test_result = test_remove_rejects_missing_code(test_directory_path);
    if (test_result != TEST_SUCCESS) {
        return test_result;
    }

    test_result = test_add_rejects_invalid_credit_count(test_directory_path);
    if (test_result != TEST_SUCCESS) {
        return test_result;
    }

    return TEST_SUCCESS;
}
