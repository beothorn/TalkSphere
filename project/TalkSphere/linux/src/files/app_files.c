#include "app_files.h"

#include "../common/app_file_names.h"
#include "../common/result.h"
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

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

#define TALKSPHERE_APPLICATION_DIRECTORY_NAME "talksphere"
#define TALKSPHERE_IDENTIFIER_FILE_NAME "id"
#define TALKSPHERE_LEDGER_DIRECTORY_NAME "ledger"
#define DEFAULT_OFFERINGS_TEXT "[\n" \
    "{\"availability\": \"alwaysOn\"},\n" \
    "{\"reachableAt\": \"www.isageek.com.br:9876\"},\n" \
    "{\"operation\":\"buy\", \"creditType\":\"own\", \"price\": 1,\"type\":\"storage\", \"offerInfo\":{\"size\": 100000, \"period\": 10}},\n" \
    "{\"operation\":\"sell\", \"creditType\":\"own\", \"price\": 1,\"type\":\"storage\", \"offerInfo\":{\"size\": 100000, \"period\": 30}},\n" \
    "{\"operation\":\"sell\", \"creditType\":\"own\", \"price\": 0.01,\"type\":\"storeMessage\", \"offerInfo\":{\"size\": 1, \"period\": 10}},\n" \
    "{\"operation\":\"sell\", \"creditType\":\"own\", \"price\": 0.001,\"type\":\"askForMessages\"},]"
#define RANDOM_IDENTIFIER_BYTES 96
#define BASE64URL_IDENTIFIER_LENGTH 128

static int directory_exists(
    const char *directory_path,
    bool *directory_found
) {
    LOG_TRACE("directory_exists(): now we check whether a directory path exists");

    struct stat directory_status;
    if (stat(directory_path, &directory_status) != 0) {
        if (errno == ENOENT) {
            *directory_found = false;
            return TALKSPHERE_SUCCESS;
        }

        LOG_ERROR("Checking a required directory failed so the startup flow cannot continue");
        return TALKSPHERE_FAILURE;
    }

    *directory_found = S_ISDIR(directory_status.st_mode);
    if (!*directory_found) {
        LOG_ERROR("The path exists but is not a directory so startup cannot proceed safely");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int create_directory_if_missing(
    const char *directory_path
) {
    LOG_TRACE("create_directory_if_missing(): now we ensure a required directory exists");

    bool directory_found = false;
    if (directory_exists(directory_path, &directory_found) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (directory_found) {
        return TALKSPHERE_SUCCESS;
    }

    LOG_INFO("Creating required application directory because it was not present");
    if (mkdir(directory_path, 0700) != 0) {
        LOG_ERROR("Could not create required directory so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int build_default_app_directory_path(
    char *app_directory_path,
    size_t app_directory_path_size
) {
    LOG_TRACE("build_default_app_directory_path(): now we compute the default Linux application directory");

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

int resolve_app_storage_directory_path(
    const char *app_storage_directory_path,
    char *resolved_directory_path,
    size_t resolved_directory_path_size
) {
    LOG_TRACE("resolve_app_storage_directory_path(): now we resolve the storage directory based on arguments");

    if (app_storage_directory_path != NULL && app_storage_directory_path[0] != '\0') {
        if (snprintf(
                resolved_directory_path,
                resolved_directory_path_size,
                "%s",
                app_storage_directory_path
            ) >= (int)resolved_directory_path_size
        ) {
            LOG_ERROR("The custom app storage directory path is too long");
            return TALKSPHERE_FAILURE;
        }
        return TALKSPHERE_SUCCESS;
    }

    return build_default_app_directory_path(
        resolved_directory_path,
        resolved_directory_path_size
    );
}

static void base64url_encode_96_bytes(
    const unsigned char random_bytes[RANDOM_IDENTIFIER_BYTES],
    char encoded_identifier[BASE64URL_IDENTIFIER_LENGTH + 1]
) {
    LOG_TRACE("base64url_encode_96_bytes(): now we encode random bytes as Base64URL without padding");

    static const char base64url_alphabet[] =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    int output_index = 0;
    for (int random_index = 0; random_index < RANDOM_IDENTIFIER_BYTES; random_index += 3) {
        unsigned int merged_bytes = ((unsigned int)random_bytes[random_index] << 16)
            | ((unsigned int)random_bytes[random_index + 1] << 8)
            | (unsigned int)random_bytes[random_index + 2];

        encoded_identifier[output_index++] = base64url_alphabet[(merged_bytes >> 18) & 0x3F];
        encoded_identifier[output_index++] = base64url_alphabet[(merged_bytes >> 12) & 0x3F];
        encoded_identifier[output_index++] = base64url_alphabet[(merged_bytes >> 6) & 0x3F];
        encoded_identifier[output_index++] = base64url_alphabet[merged_bytes & 0x3F];
    }

    encoded_identifier[output_index] = '\0';
}

static int build_identifier_file_path(
    const char *app_directory_path,
    char *identifier_file_path,
    size_t identifier_file_path_size
) {
    LOG_TRACE("build_identifier_file_path(): now we compute the identifier file path");

    if (snprintf(
            identifier_file_path,
            identifier_file_path_size,
            "%s/%s",
            app_directory_path,
            TALKSPHERE_IDENTIFIER_FILE_NAME
        ) >= (int)identifier_file_path_size
    ) {
        LOG_ERROR("The identifier file path is too long so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int build_offerings_file_path(
    const char *app_directory_path,
    char *offerings_file_path,
    size_t offerings_file_path_size
) {
    LOG_TRACE("build_offerings_file_path(): now we compute where the local offerings document lives");

    if (snprintf(
            offerings_file_path,
            offerings_file_path_size,
            "%s/%s",
            app_directory_path,
            TALKSPHERE_OFFERINGS_FILE_NAME
        ) >= (int)offerings_file_path_size
    ) {
        LOG_ERROR("The offerings file path is too long so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int ensure_identifier_file(
    const char *app_directory_path
) {
    LOG_TRACE("ensure_identifier_file(): now we create the installation identifier if it does not exist yet");

    char identifier_file_path[PATH_MAX];
    if (build_identifier_file_path(
            app_directory_path,
            identifier_file_path,
            sizeof(identifier_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
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
    ssize_t random_bytes_read_count = getrandom(random_bytes, sizeof(random_bytes), 0);
    if (random_bytes_read_count != (ssize_t)sizeof(random_bytes)) {
        LOG_ERROR("Cryptographic random generation failed so identifier creation cannot continue");
        close(identifier_file_descriptor);
        return TALKSPHERE_FAILURE;
    }

    char encoded_identifier[BASE64URL_IDENTIFIER_LENGTH + 1];
    base64url_encode_96_bytes(random_bytes, encoded_identifier);

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

static int ensure_offerings_file(
    const char *app_directory_path
) {
    LOG_TRACE("ensure_offerings_file(): now we create the default local offerings document if it does not exist yet");

    char offerings_file_path[PATH_MAX];
    if (build_offerings_file_path(
            app_directory_path,
            offerings_file_path,
            sizeof(offerings_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    int offerings_file_descriptor = open(
        offerings_file_path,
        O_WRONLY | O_CREAT | O_EXCL,
        0600
    );

    if (offerings_file_descriptor < 0) {
        if (errno == EEXIST) {
            return TALKSPHERE_SUCCESS;
        }

        LOG_ERROR("Opening the offerings file failed so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    const char *default_offerings_text = DEFAULT_OFFERINGS_TEXT;
    size_t default_offerings_text_length = strlen(default_offerings_text);
    ssize_t written_bytes_count = write(
        offerings_file_descriptor,
        default_offerings_text,
        default_offerings_text_length
    );

    close(offerings_file_descriptor);

    if (written_bytes_count != (ssize_t)default_offerings_text_length) {
        LOG_ERROR("Writing the default offerings failed so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    LOG_INFO("A default local offerings document was created");
    return TALKSPHERE_SUCCESS;
}

int ensure_app_files(
    const char *app_storage_directory_path
) {
    LOG_TRACE("ensure_app_files(): now we ensure required app files and directories exist before network startup");

    char resolved_directory_path[PATH_MAX];
    if (resolve_app_storage_directory_path(
            app_storage_directory_path,
            resolved_directory_path,
            sizeof(resolved_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (create_directory_if_missing(resolved_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (ensure_identifier_file(resolved_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (ensure_offerings_file(resolved_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    char ledger_directory_path[PATH_MAX];
    if (snprintf(
            ledger_directory_path,
            sizeof(ledger_directory_path),
            "%s/%s",
            resolved_directory_path,
            TALKSPHERE_LEDGER_DIRECTORY_NAME
        ) >= (int)sizeof(ledger_directory_path)
    ) {
        LOG_ERROR("The ledger directory path is too long so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return create_directory_if_missing(ledger_directory_path);
}

int read_local_identifier(
    const char *app_storage_directory_path,
    char *identifier_text,
    int identifier_text_size
) {
    LOG_TRACE("read_local_identifier(): now we load the local identifier from the storage directory");

    char resolved_directory_path[PATH_MAX];
    if (resolve_app_storage_directory_path(
            app_storage_directory_path,
            resolved_directory_path,
            sizeof(resolved_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char identifier_file_path[PATH_MAX];
    if (build_identifier_file_path(
            resolved_directory_path,
            identifier_file_path,
            sizeof(identifier_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    FILE *identifier_file = fopen(identifier_file_path, "r");
    if (identifier_file == NULL) {
        LOG_ERROR("Opening identifier file failed so we cannot process credit commands");
        return TALKSPHERE_FAILURE;
    }

    if (fgets(identifier_text, identifier_text_size, identifier_file) == NULL) {
        fclose(identifier_file);
        LOG_ERROR("Reading identifier file failed so we cannot process credit commands");
        return TALKSPHERE_FAILURE;
    }

    fclose(identifier_file);
    identifier_text[strcspn(identifier_text, "\r\n")] = '\0';
    return TALKSPHERE_SUCCESS;
}
