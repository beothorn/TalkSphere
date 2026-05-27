#include "app_files.h"

#include "../logging.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/random.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

#define TALKSPHERE_APPLICATION_DIRECTORY_NAME "talksphere"
#define TALKSPHERE_IDENTIFIER_FILE_NAME "id"
#define RANDOM_IDENTIFIER_BYTES 16
#define BASE64URL_IDENTIFIER_LENGTH 22

static int directory_exists(
    const char *directory_path,
    bool *directory_found
) {
    LOG_TRACE("directory_exists(): now we check whether the app data directory already exists");

    struct stat directory_status;
    if (stat(directory_path, &directory_status) != 0) {
        if (errno == ENOENT) {
            *directory_found = false;
            return TALKSPHERE_SUCCESS;
        }

        LOG_ERROR("Checking the app data directory failed so the startup flow cannot continue");
        return TALKSPHERE_FAILURE;
    }

    *directory_found = S_ISDIR(directory_status.st_mode);

    if (!*directory_found) {
        LOG_ERROR("The app data path exists but is not a directory so startup cannot proceed safely");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int build_app_directory_path(
    char *app_directory_path,
    size_t app_directory_path_size
) {
    LOG_TRACE("build_app_directory_path(): now we compute where Linux application files should live");

    const char *xdg_data_home = getenv("XDG_DATA_HOME");
    const char *home_directory = getenv("HOME");

    if (xdg_data_home != NULL && xdg_data_home[0] != '\0') {
        if (snprintf(
                app_directory_path,
                app_directory_path_size,
                "%s/%s",
                xdg_data_home,
                TALKSPHERE_APPLICATION_DIRECTORY_NAME
            ) >= (int)app_directory_path_size
        ) {
            LOG_ERROR("The app directory path is too long when using XDG_DATA_HOME");
            return TALKSPHERE_FAILURE;
        }
        return TALKSPHERE_SUCCESS;
    }

    if (home_directory == NULL || home_directory[0] == '\0') {
        LOG_ERROR("HOME is not available so we cannot resolve the Linux application files directory");
        return TALKSPHERE_FAILURE;
    }

    if (snprintf(
            app_directory_path,
            app_directory_path_size,
            "%s/.local/share/%s",
            home_directory,
            TALKSPHERE_APPLICATION_DIRECTORY_NAME
        ) >= (int)app_directory_path_size
    ) {
        LOG_ERROR("The app directory path is too long when using HOME fallback");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int create_directory_if_missing(
    const char *directory_path
) {
    LOG_TRACE("create_directory_if_missing(): now we ensure the app data directory exists");

    bool directory_found = false;
    if (directory_exists(directory_path, &directory_found) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (directory_found) {
        return TALKSPHERE_SUCCESS;
    }

    LOG_INFO("Creating app data directory because it was not present");
    if (mkdir(directory_path, 0700) != 0) {
        LOG_ERROR("Could not create the app data directory so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static void base64url_encode_16_bytes(
    const unsigned char random_bytes[RANDOM_IDENTIFIER_BYTES],
    char encoded_identifier[BASE64URL_IDENTIFIER_LENGTH + 1]
) {
    LOG_TRACE("base64url_encode_16_bytes(): now we encode random bytes as Base64URL without padding");

    static const char base64url_alphabet[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    size_t output_index = 0;
    for (size_t random_index = 0; random_index < 15; random_index += 3) {
        unsigned int merged_bytes = ((unsigned int)random_bytes[random_index] << 16)
            | ((unsigned int)random_bytes[random_index + 1] << 8)
            | (unsigned int)random_bytes[random_index + 2];

        encoded_identifier[output_index++] = base64url_alphabet[(merged_bytes >> 18) & 0x3F];
        encoded_identifier[output_index++] = base64url_alphabet[(merged_bytes >> 12) & 0x3F];
        encoded_identifier[output_index++] = base64url_alphabet[(merged_bytes >> 6) & 0x3F];
        encoded_identifier[output_index++] = base64url_alphabet[merged_bytes & 0x3F];
    }

    unsigned int final_group = ((unsigned int)random_bytes[15] << 16);
    encoded_identifier[output_index++] = base64url_alphabet[(final_group >> 18) & 0x3F];
    encoded_identifier[output_index++] = base64url_alphabet[(final_group >> 12) & 0x3F];
    encoded_identifier[output_index] = '\0';
}

static int ensure_identifier_file(
    const char *app_directory_path
) {
    LOG_TRACE("ensure_identifier_file(): now we create the installation identifier if it does not exist yet");

    char identifier_file_path[PATH_MAX];
    if (snprintf(
            identifier_file_path,
            sizeof(identifier_file_path),
            "%s/%s",
            app_directory_path,
            TALKSPHERE_IDENTIFIER_FILE_NAME
        ) >= (int)sizeof(identifier_file_path)
    ) {
        LOG_ERROR("The identifier file path is too long so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    int identifier_file_descriptor = open(
        identifier_file_path,
        O_WRONLY | O_CREAT | O_EXCL,
        0600
    );

    if (identifier_file_descriptor < 0) {
        if (errno == EEXIST) {
            return TALKSPHERE_SUCCESS;
        }

        LOG_ERROR("Opening the identifier file failed so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    unsigned char random_bytes[RANDOM_IDENTIFIER_BYTES];
    ssize_t random_bytes_read_count = getrandom(
        random_bytes,
        sizeof(random_bytes),
        0
    );

    if (random_bytes_read_count != (ssize_t)sizeof(random_bytes)) {
        LOG_ERROR("Cryptographic random generation failed so identifier creation cannot continue");
        close(identifier_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    char encoded_identifier[BASE64URL_IDENTIFIER_LENGTH + 1];
    base64url_encode_16_bytes(random_bytes, encoded_identifier);

    size_t encoded_identifier_length = strlen(encoded_identifier);
    ssize_t written_bytes_count = write(
        identifier_file_descriptor,
        encoded_identifier,
        encoded_identifier_length
    );

    close(identifier_file_descriptor);

    if (written_bytes_count != (ssize_t)encoded_identifier_length) {
        LOG_ERROR("Writing the identifier failed so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    LOG_INFO("A new installation identifier was created");
    return TALKSPHERE_SUCCESS;
}

int ensure_app_files(void) {
    LOG_TRACE("ensure_app_files(): now we ensure required Linux app files exist before network startup");

    char app_directory_path[PATH_MAX];
    if (build_app_directory_path(app_directory_path, sizeof(app_directory_path)) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (create_directory_if_missing(app_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return ensure_identifier_file(app_directory_path);
}
