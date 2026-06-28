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
#define DEFAULT_OFFERINGS_FILE_PATH "defaults/offerings.json"
#define REPOSITORY_DEFAULT_OFFERINGS_FILE_PATH "project/TalkSphere/linux/defaults/offerings.json"
#define RANDOM_IDENTIFIER_BYTES 96
#define BASE64URL_IDENTIFIER_LENGTH 128
#define FILE_COPY_BUFFER_SIZE 4096

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
    LOG_TRACE(">build_default_app_directory_path(): now we compute the default Linux application directory");

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
            LOG_TRACE("<build_default_app_directory_path(): failed to compute default app directory path");
            return TALKSPHERE_FAILURE;
        }
        LOG_TRACE("<build_default_app_directory_path(): successfully computed default app directory path");
        return TALKSPHERE_SUCCESS;
    }

    if (home_directory == NULL || home_directory[0] == '\0') {
        LOG_ERROR("HOME is not available so we cannot resolve the Linux application files directory");
        LOG_TRACE("<build_default_app_directory_path(): failed to compute default app directory path");
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
        LOG_TRACE("<build_default_app_directory_path(): failed to compute default app directory path");
        return TALKSPHERE_FAILURE;
    }

    LOG_TRACE("<build_default_app_directory_path(): successfully computed default app directory path");
    return TALKSPHERE_SUCCESS;
}

int resolve_app_storage_directory_path(
    const char *app_storage_directory_path,
    char *resolved_directory_path,
    size_t resolved_directory_path_size
) {
    LOG_TRACE(">resolve_app_storage_directory_path(): now we resolve the storage directory based on arguments");

    if (app_storage_directory_path != NULL && app_storage_directory_path[0] != '\0') {
        if (snprintf(
                resolved_directory_path,
                resolved_directory_path_size,
                "%s",
                app_storage_directory_path
            ) >= (int)resolved_directory_path_size
        ) {
            LOG_ERROR("The custom app storage directory path is too long");
            LOG_TRACE("<resolve_app_storage_directory_path(): failed to resolve storage directory path");
            return TALKSPHERE_FAILURE;
        }
        LOG_TRACE("<resolve_app_storage_directory_path(): successfully resolved storage directory path");
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
    LOG_TRACE(">build_identifier_file_path(): now we compute the identifier file path");

    if (snprintf(
            identifier_file_path,
            identifier_file_path_size,
            "%s/%s",
            app_directory_path,
            TALKSPHERE_IDENTIFIER_FILE_NAME
        ) >= (int)identifier_file_path_size
    ) {
        LOG_ERROR("The identifier file path is too long so startup cannot continue");
        LOG_TRACE("<build_identifier_file_path(): failed to compute identifier file path");
        return TALKSPHERE_FAILURE;
    }

    LOG_TRACE("<build_identifier_file_path(): successfully computed identifier file path");
    return TALKSPHERE_SUCCESS;
}

static int build_offerings_file_path(
    const char *app_directory_path,
    char *offerings_file_path,
    size_t offerings_file_path_size
) {
    LOG_TRACE(">build_offerings_file_path(): now we compute where the local offerings document lives");

    if (snprintf(
            offerings_file_path,
            offerings_file_path_size,
            "%s/%s",
            app_directory_path,
            TALKSPHERE_OFFERINGS_FILE_NAME
        ) >= (int)offerings_file_path_size
    ) {
        LOG_ERROR("The offerings file path is too long so startup cannot continue");
        LOG_TRACE("<build_offerings_file_path(): failed to compute offerings file path");
        return TALKSPHERE_FAILURE;
    }

    LOG_TRACE("<build_offerings_file_path(): successfully computed offerings file path");
    return TALKSPHERE_SUCCESS;
}

static int build_config_file_path(
    const char *app_directory_path,
    char *config_file_path,
    size_t config_file_path_size
) {
    LOG_TRACE(">build_config_file_path(): now we compute where the local config document lives");

    if (snprintf(
            config_file_path,
            config_file_path_size,
            "%s/%s",
            app_directory_path,
            TALKSPHERE_CONFIG_FILE_NAME
        ) >= (int)config_file_path_size
    ) {
        LOG_ERROR("The config file path is too long so startup cannot continue");
        LOG_TRACE("<build_config_file_path(): failed to compute config file path");
        return TALKSPHERE_FAILURE;
    }

    LOG_TRACE("<build_config_file_path(): successfully computed config file path");
    return TALKSPHERE_SUCCESS;
}

static int ensure_identifier_file(
    const char *app_directory_path
) {
    LOG_TRACE(">ensure_identifier_file(): now we create the installation identifier if it does not exist yet");

    char identifier_file_path[PATH_MAX];
    if (build_identifier_file_path(
            app_directory_path,
            identifier_file_path,
            sizeof(identifier_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        LOG_TRACE("<ensure_identifier_file(): failed to build identifier file path");
        return TALKSPHERE_FAILURE;
    }

    int identifier_file_descriptor = open(
        identifier_file_path,
        O_WRONLY | O_CREAT | O_EXCL,
        0600
    );

    if (identifier_file_descriptor < 0) {
        if (errno == EEXIST) {
            LOG_TRACE("<ensure_identifier_file(): identifier file already exists");
            return TALKSPHERE_SUCCESS;
        }

        LOG_TRACE("<ensure_identifier_file(): failed to open identifier file");
        LOG_ERROR("Opening the identifier file failed so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    unsigned char random_bytes[RANDOM_IDENTIFIER_BYTES];
    ssize_t random_bytes_read_count = getrandom(random_bytes, sizeof(random_bytes), 0);
    if (random_bytes_read_count != (ssize_t)sizeof(random_bytes)) {
        LOG_ERROR("Cryptographic random generation failed so identifier creation cannot continue");
        close(identifier_file_descriptor);
        LOG_TRACE("<ensure_identifier_file(): failed to generate random bytes");
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
        LOG_TRACE("<ensure_identifier_file(): failed to write identifier to file");
        return TALKSPHERE_FAILURE;
    }

    LOG_INFO("A new installation identifier was created");

    printf(
        "A new identifier was created: %s\n",
        encoded_identifier
    );


    LOG_TRACE("<ensure_identifier_file(): successfully created identifier file");
    return TALKSPHERE_SUCCESS;
}

static FILE *open_default_offerings_file(void) {
    LOG_TRACE("open_default_offerings_file(): now we open the default offerings document shipped with the application");

    FILE *default_offerings_file = fopen(
        DEFAULT_OFFERINGS_FILE_PATH,
        "r"
    );
    if (default_offerings_file != NULL) {
        return default_offerings_file;
    }

    return fopen(
        REPOSITORY_DEFAULT_OFFERINGS_FILE_PATH,
        "r"
    );
}

static int write_all_bytes(
    int file_descriptor,
    const char *file_text,
    size_t file_text_length
) {
    LOG_TRACE("write_all_bytes(): now we keep writing until the whole file chunk is stored");

    size_t written_total_count = 0;
    while (written_total_count < file_text_length) {
        ssize_t written_bytes_count = write(
            file_descriptor,
            file_text + written_total_count,
            file_text_length - written_total_count
        );

        if (written_bytes_count <= 0) {
            LOG_ERROR("Writing bytes failed so the file cannot be completed");
            return TALKSPHERE_FAILURE;
        }

        written_total_count += (size_t)written_bytes_count;
    }

    return TALKSPHERE_SUCCESS;
}

static int copy_default_offerings_file(
    int offerings_file_descriptor
) {
    LOG_TRACE("copy_default_offerings_file(): now we copy the shipped default offerings into the local app file");

    FILE *default_offerings_file = open_default_offerings_file();
    if (default_offerings_file == NULL) {
        LOG_ERROR("Opening the default offerings asset failed so startup cannot create the local offerings file");
        return TALKSPHERE_FAILURE;
    }

    char file_copy_buffer[FILE_COPY_BUFFER_SIZE];
    while (true) {
        size_t read_bytes_count = fread(
            file_copy_buffer,
            sizeof(char),
            sizeof(file_copy_buffer),
            default_offerings_file
        );

        if (read_bytes_count > 0) {
            if (write_all_bytes(
                offerings_file_descriptor,
                file_copy_buffer,
                read_bytes_count
            ) != TALKSPHERE_SUCCESS
            ) {
                fclose(default_offerings_file);
                LOG_ERROR("Writing the default offerings failed so startup cannot continue");
                return TALKSPHERE_FAILURE;
            }
        }

        if (read_bytes_count < sizeof(file_copy_buffer)) {
            if (ferror(default_offerings_file)) {
                fclose(default_offerings_file);
                LOG_ERROR("Reading the default offerings asset failed so startup cannot create the local offerings file");
                return TALKSPHERE_FAILURE;
            }

            break;
        }
    }

    fclose(default_offerings_file);
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

    int copy_default_offerings_result = copy_default_offerings_file(
        offerings_file_descriptor
    );

    close(offerings_file_descriptor);

    if (copy_default_offerings_result != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    LOG_INFO("A default local offerings document was created");
    return TALKSPHERE_SUCCESS;
}

static int ensure_config_file(
    const char *app_directory_path
) {
    LOG_TRACE("ensure_config_file(): now we create the local config document if it does not exist yet");

    char config_file_path[PATH_MAX];
    if (build_config_file_path(
            app_directory_path,
            config_file_path,
            sizeof(config_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    int config_file_descriptor = open(
        config_file_path,
        O_WRONLY | O_CREAT | O_EXCL,
        0600
    );

    if (config_file_descriptor < 0) {
        if (errno == EEXIST) {
            return TALKSPHERE_SUCCESS;
        }

        LOG_ERROR("Opening the config file failed so startup cannot continue");
        return TALKSPHERE_FAILURE;
    }

    close(config_file_descriptor);
    LOG_INFO("A local config document was created");
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

    if (ensure_config_file(resolved_directory_path) != TALKSPHERE_SUCCESS) {
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
