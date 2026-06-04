#include "files/app_files.h"
#include "common/result.h"

#include <ctype.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#define IDENTIFIER_TEXT_SIZE 256
#define PATH_TEXT_SIZE 512

static int path_exists(
    const char *path
) {
    struct stat path_status;

    return stat(
        path,
        &path_status
    ) == 0;
}

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

static int identifier_has_expected_shape(
    const char *identifier_text
) {
    size_t identifier_text_length = strlen(identifier_text);
    if (identifier_text_length != 128) {
        return 0;
    }

    for (size_t character_index = 0; character_index < identifier_text_length; character_index++) {
        unsigned char identifier_character = (unsigned char)identifier_text[character_index];
        if (!isalnum(identifier_character)
            && identifier_character != '-'
            && identifier_character != '_'
        ) {
            return 0;
        }
    }

    return 1;
}

static int offerings_have_expected_defaults(
    const char *app_storage_directory_path
) {
    char offerings_file_path[PATH_TEXT_SIZE];
    char offerings_text[1024];

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
        "r"
    );
    if (offerings_file == NULL) {
        return TALKSPHERE_FAILURE;
    }

    size_t read_bytes_count = fread(
        offerings_text,
        sizeof(char),
        sizeof(offerings_text) - 1,
        offerings_file
    );
    fclose(offerings_file);
    offerings_text[read_bytes_count] = '\0';

    return strstr(
        offerings_text,
        "\"availability\": \"alwaysOn\""
    ) != NULL
        && strstr(
            offerings_text,
            "\"type\":\"askForMessages\""
        ) != NULL
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

int main(void) {
    char temporary_directory_path[PATH_TEXT_SIZE];
    char app_storage_directory_path[PATH_TEXT_SIZE];
    char ledger_directory_path[PATH_TEXT_SIZE];
    char identifier_file_path[PATH_TEXT_SIZE];
    char offerings_file_path[PATH_TEXT_SIZE];
    char identifier_text[IDENTIFIER_TEXT_SIZE];

    if (snprintf(
            temporary_directory_path,
            sizeof(temporary_directory_path),
            "/tmp/talksphere-files-test-%ld",
            (long)getpid()
        ) >= (int)sizeof(temporary_directory_path)
    ) {
        return 1;
    }

    if (mkdir(
            temporary_directory_path,
            0700
        ) != 0
    ) {
        return 11;
    }

    if (build_path(
            temporary_directory_path,
            "app",
            app_storage_directory_path,
            sizeof(app_storage_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return 2;
    }

    if (ensure_app_files(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return 3;
    }

    if (build_path(
            app_storage_directory_path,
            "ledger",
            ledger_directory_path,
            sizeof(ledger_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return 4;
    }

    if (build_path(
            app_storage_directory_path,
            "id",
            identifier_file_path,
            sizeof(identifier_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return 5;
    }

    if (build_path(
            app_storage_directory_path,
            "offerings",
            offerings_file_path,
            sizeof(offerings_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return 6;
    }

    if (!path_exists(app_storage_directory_path)
        || !path_exists(ledger_directory_path)
        || !path_exists(identifier_file_path)
        || !path_exists(offerings_file_path)
    ) {
        return 7;
    }

    if (read_local_identifier(
            app_storage_directory_path,
            identifier_text,
            sizeof(identifier_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return 8;
    }

    if (!identifier_has_expected_shape(identifier_text)) {
        return 9;
    }

    if (offerings_have_expected_defaults(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return 10;
    }

    return 0;
}
