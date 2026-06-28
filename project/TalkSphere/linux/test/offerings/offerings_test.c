#include "offerings/offerings.h"
#include "common/result.h"
#include "test_support.h"

#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define PATH_TEXT_SIZE 512

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

static int write_offerings_file(
    const char *app_storage_directory_path
) {
    char offerings_file_path[PATH_TEXT_SIZE];

    if (build_path(
            app_storage_directory_path,
            "offerings",
            offerings_file_path,
            sizeof(offerings_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    FILE *offerings_file = fopen(
        offerings_file_path,
        "w"
    );
    if (offerings_file == NULL) {
        return TALKSPHERE_FAILURE;
    }

    if (fprintf(
            offerings_file,
            "[{\"availability\":\"alwaysOn\"}]"
        ) < 0
    ) {
        fclose(offerings_file);
        return TALKSPHERE_FAILURE;
    }

    fclose(offerings_file);
    return TALKSPHERE_SUCCESS;
}

static int test_read_local_offerings(void) {
    char temporary_directory_path[PATH_TEXT_SIZE];
    char app_storage_directory_path[PATH_TEXT_SIZE];
    char missing_file_directory_path[PATH_TEXT_SIZE];
    char offerings_text[256];
    char small_offerings_text[4];

    if (snprintf(
            temporary_directory_path,
            sizeof(temporary_directory_path),
            "/tmp/talksphere-offerings-test-%ld",
            (long)getpid()
        ) >= (int)sizeof(temporary_directory_path)
    ) {
        return TEST_FAILURE;
    }

    if (mkdir(
            temporary_directory_path,
            0700
        ) != 0
    ) {
        return TEST_FAILURE;
    }

    if (build_path(
            temporary_directory_path,
            "app",
            app_storage_directory_path,
            sizeof(app_storage_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE;
    }

    if (mkdir(
            app_storage_directory_path,
            0700
        ) != 0
    ) {
        return TEST_FAILURE;
    }

    if (write_offerings_file(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return TEST_FAILURE;
    }

    if (read_local_offerings(
            app_storage_directory_path,
            offerings_text,
            strlen("[{\"availability\":\"alwaysOn\"}]") + 1
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE;
    }

    if (strcmp(
            offerings_text,
            "[{\"availability\":\"alwaysOn\"}]"
        ) != 0
    ) {
        return TEST_FAILURE;
    }

    if (build_path(
            temporary_directory_path,
            "missing",
            missing_file_directory_path,
            sizeof(missing_file_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE;
    }

    if (read_local_offerings(
            missing_file_directory_path,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE;
    }

    if (read_local_offerings(
            app_storage_directory_path,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TEST_FAILURE;
    }

    if (read_local_offerings(
            app_storage_directory_path,
            small_offerings_text,
            sizeof(small_offerings_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE;
    }

    if (read_local_offerings(
            NULL,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE;
    }

    if (read_local_offerings(
            app_storage_directory_path,
            NULL,
            sizeof(offerings_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE;
    }

    if (read_local_offerings(
            app_storage_directory_path,
            offerings_text,
            0
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE;
    }

    return TEST_SUCCESS;
}

int main(void) {
    const struct test_case test_cases[] = {
        TEST_CASE(test_read_local_offerings)
    };

    return run_test_cases(
        test_cases,
        sizeof(test_cases) / sizeof(test_cases[0])
    );
}
