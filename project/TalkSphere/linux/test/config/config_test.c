#include "config/config.h"
#include "common/result.h"
#include "test_support.h"

#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define PATH_TEXT_SIZE 512
#define CONFIG_TEXT_SIZE 2048
#define CONFIG_FILE_NAME "config"
#define EXPECTED_INITIAL_CONFIG_TEXT "availability=alwaysOn\nreachableAt=www.example.com:9999\n"
#define EXPECTED_REPLACED_CONFIG_TEXT "reachableAt=www.example.com:9999\navailability=manual\n"
#define EXPECTED_REMOVED_CONFIG_TEXT "availability=manual\n"
#define TEST_FAILURE_PREPARE_DIRECTORY 1
#define TEST_FAILURE_SET_INITIAL_AVAILABILITY 2
#define TEST_FAILURE_ADD_REACHABLE_ADDRESS 3
#define TEST_FAILURE_INITIAL_CONTENT 4
#define TEST_FAILURE_REPLACE_AVAILABILITY 5
#define TEST_FAILURE_REPLACED_CONTENT 6
#define TEST_FAILURE_DUPLICATE_REACHABLE_ADDRESS 7
#define TEST_FAILURE_REMOVE_REACHABLE_ADDRESS 8
#define TEST_FAILURE_REMOVED_CONTENT 9
#define TEST_FAILURE_REMOVE_MISSING_ADDRESS 10
#define TEST_FAILURE_SET_LIST_KEY 11
#define TEST_FAILURE_ADD_SCALAR_KEY 12
#define TEST_FAILURE_GET_AVAILABILITY 13
#define TEST_FAILURE_GET_MISSING_AVAILABILITY 14

static int build_path(
    const char *directory_path,
    const char *file_name,
    char *path,
    size_t path_size
) {
    return snprintf(
        path,
        path_size,
        "%s/%s",
        directory_path,
        file_name
    ) < (int)path_size
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int create_empty_config_file(
    const char *app_storage_directory_path
) {
    char config_file_path[PATH_TEXT_SIZE];
    if (build_path(
            app_storage_directory_path,
            CONFIG_FILE_NAME,
            config_file_path,
            sizeof(config_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    FILE *config_file = fopen(
        config_file_path,
        "w"
    );
    if (config_file == NULL) {
        return TALKSPHERE_FAILURE;
    }

    fclose(config_file);
    return TALKSPHERE_SUCCESS;
}

static int read_config_file(
    const char *app_storage_directory_path,
    char *config_text,
    size_t config_text_size
) {
    char config_file_path[PATH_TEXT_SIZE];
    if (build_path(
            app_storage_directory_path,
            CONFIG_FILE_NAME,
            config_file_path,
            sizeof(config_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    FILE *config_file = fopen(
        config_file_path,
        "r"
    );
    if (config_file == NULL) {
        return TALKSPHERE_FAILURE;
    }

    size_t read_bytes_count = fread(
        config_text,
        sizeof(char),
        config_text_size - 1,
        config_file
    );
    fclose(config_file);

    config_text[read_bytes_count] = '\0';
    return TALKSPHERE_SUCCESS;
}

static int config_file_equals(
    const char *app_storage_directory_path,
    const char *expected_config_text
) {
    char config_text[CONFIG_TEXT_SIZE];
    if (read_config_file(
            app_storage_directory_path,
            config_text,
            sizeof(config_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return strcmp(
        config_text,
        expected_config_text
    ) == 0
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int prepare_test_directory(
    char *temporary_directory_path,
    size_t temporary_directory_path_size
) {
    if (snprintf(
            temporary_directory_path,
            temporary_directory_path_size,
            "/tmp/talksphere-config-test-%ld",
            (long)getpid()
        ) >= (int)temporary_directory_path_size
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (mkdir(
            temporary_directory_path,
            0700
        ) != 0
    ) {
        return TALKSPHERE_FAILURE;
    }

    return create_empty_config_file(temporary_directory_path);
}

static int test_config_value_lifecycle(void) {
    char temporary_directory_path[PATH_TEXT_SIZE];
    if (prepare_test_directory(
            temporary_directory_path,
            sizeof(temporary_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_PREPARE_DIRECTORY;
    }

    if (config_set_value(
            temporary_directory_path,
            "availability",
            "alwaysOn"
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_SET_INITIAL_AVAILABILITY;
    }

    if (config_add_value(
            temporary_directory_path,
            "reachableAt",
            "www.example.com:9999"
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_ADD_REACHABLE_ADDRESS;
    }

    if (config_file_equals(
            temporary_directory_path,
            EXPECTED_INITIAL_CONFIG_TEXT
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_INITIAL_CONTENT;
    }

    if (config_set_value(
            temporary_directory_path,
            "availability",
            "manual"
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_REPLACE_AVAILABILITY;
    }

    if (config_file_equals(
            temporary_directory_path,
            EXPECTED_REPLACED_CONFIG_TEXT
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_REPLACED_CONTENT;
    }

    if (config_add_value(
            temporary_directory_path,
            "reachableAt",
            "www.example.com:9999"
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE_DUPLICATE_REACHABLE_ADDRESS;
    }

    if (config_remove_value(
            temporary_directory_path,
            "reachableAt",
            "www.example.com:9999"
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_REMOVE_REACHABLE_ADDRESS;
    }

    if (config_file_equals(
            temporary_directory_path,
            EXPECTED_REMOVED_CONFIG_TEXT
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_REMOVED_CONTENT;
    }

    if (config_remove_value(
            temporary_directory_path,
            "reachableAt",
            "www.example.com:9999"
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE_REMOVE_MISSING_ADDRESS;
    }

    if (config_set_value(
            temporary_directory_path,
            "reachableAt",
            "www.example.com:9999"
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE_SET_LIST_KEY;
    }

    if (config_add_value(
            temporary_directory_path,
            "availability",
            "alwaysOn"
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE_ADD_SCALAR_KEY;
    }

    char config_value_text[CONFIG_TEXT_SIZE];
    if (config_get_value(
            temporary_directory_path,
            "availability",
            config_value_text,
            sizeof(config_value_text)
        ) != TALKSPHERE_SUCCESS
        || strcmp(
            config_value_text,
            "manual"
        ) != 0
    ) {
        return TEST_FAILURE_GET_AVAILABILITY;
    }

    if (create_empty_config_file(temporary_directory_path) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE_PREPARE_DIRECTORY;
    }

    if (config_get_value(
            temporary_directory_path,
            "availability",
            config_value_text,
            sizeof(config_value_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE_GET_MISSING_AVAILABILITY;
    }

    return TEST_SUCCESS;
}

int main(void) {
    const struct test_case test_cases[] = {
        TEST_CASE(test_config_value_lifecycle)
    };

    return run_test_cases(
        test_cases,
        sizeof(test_cases) / sizeof(test_cases[0])
    );
}
