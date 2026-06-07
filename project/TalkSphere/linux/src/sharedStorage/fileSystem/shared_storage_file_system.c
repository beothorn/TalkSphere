#include "shared_storage_file_system.h"

#include "../../common/result.h"
#include "../../logging.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/stat.h>
#include <unistd.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

#define TALKSPHERE_APPLICATION_DIRECTORY_NAME "talksphere"
#define SHARED_STORAGE_DIRECTORY_NAME "sharedStorage"
#define SHARED_STORAGE_FILE_DIRECTORY_NAME "files"
#define SHARED_STORAGE_DATABASE_FILE_NAME "file_manager.sqlite"

static int path_exists_as_directory(
    const char *directory_path,
    bool *directory_found
) {
    LOG_TRACE("path_exists_as_directory(): now we check whether a shared storage directory already exists");

    struct stat directory_status;
    if (stat(
            directory_path,
            &directory_status
        ) != 0
    ) {
        if (errno == ENOENT) {
            *directory_found = false;
            return TALKSPHERE_SUCCESS;
        }

        LOG_ERROR("Checking a shared storage directory failed so file storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    *directory_found = S_ISDIR(directory_status.st_mode);
    if (!*directory_found) {
        LOG_ERROR("The shared storage path exists but is not a directory so file storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int create_directory_if_missing(
    const char *directory_path
) {
    LOG_TRACE("create_directory_if_missing(): now we ensure one shared storage directory exists");

    bool directory_found = false;
    if (path_exists_as_directory(
            directory_path,
            &directory_found
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (directory_found) {
        return TALKSPHERE_SUCCESS;
    }

    LOG_INFO("Creating shared storage directory because the file manager needs it before saving files");
    if (mkdir(
            directory_path,
            0700
        ) != 0
    ) {
        LOG_ERROR("Creating a shared storage directory failed so file storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int build_default_app_storage_directory_path(
    char *app_storage_directory_path,
    size_t app_storage_directory_path_size
) {
    LOG_TRACE("build_default_app_storage_directory_path(): now we resolve the default app storage directory from HOME");

    const char *home_directory = getenv("HOME");
    if (home_directory == NULL || home_directory[0] == '\0') {
        LOG_ERROR("HOME is unavailable so shared storage cannot choose its default home-folder location");
        return TALKSPHERE_FAILURE;
    }

    if (snprintf(
            app_storage_directory_path,
            app_storage_directory_path_size,
            "%s/.local/share/%s",
            home_directory,
            TALKSPHERE_APPLICATION_DIRECTORY_NAME
        ) >= (int)app_storage_directory_path_size
    ) {
        LOG_ERROR("The default shared storage app directory path is too long");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int resolve_app_storage_directory_path(
    const char *requested_app_storage_directory_path,
    char *app_storage_directory_path,
    size_t app_storage_directory_path_size
) {
    LOG_TRACE("resolve_app_storage_directory_path(): now we choose between caller storage root and the default home-folder root");

    if (requested_app_storage_directory_path != NULL && requested_app_storage_directory_path[0] != '\0') {
        if (snprintf(
                app_storage_directory_path,
                app_storage_directory_path_size,
                "%s",
                requested_app_storage_directory_path
            ) >= (int)app_storage_directory_path_size
        ) {
            LOG_ERROR("The requested shared storage app directory path is too long");
            return TALKSPHERE_FAILURE;
        }

        return TALKSPHERE_SUCCESS;
    }

    return build_default_app_storage_directory_path(
        app_storage_directory_path,
        app_storage_directory_path_size
    );
}

static int append_path_part(
    const char *base_directory_path,
    const char *path_part,
    char *joined_path,
    size_t joined_path_size
) {
    LOG_TRACE("append_path_part(): now we join two path parts for shared storage");

    if (snprintf(
            joined_path,
            joined_path_size,
            "%s/%s",
            base_directory_path,
            path_part
        ) >= (int)joined_path_size
    ) {
        LOG_ERROR("A shared storage path is too long");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int append_text_as_hex(
    const char *text,
    char *encoded_text,
    size_t encoded_text_size
) {
    LOG_TRACE("append_text_as_hex(): now we encode untrusted identifiers so they cannot escape the storage folder");

    static const char hexadecimal_digits[] = "0123456789abcdef";

    size_t encoded_text_index = 0;
    for (size_t text_index = 0; text[text_index] != '\0'; text_index++) {
        if (encoded_text_index + 2 >= encoded_text_size) {
            LOG_ERROR("The encoded shared storage identifier is too long for a file name");
            return TALKSPHERE_FAILURE;
        }

        unsigned char text_byte = (unsigned char)text[text_index];
        encoded_text[encoded_text_index++] = hexadecimal_digits[(text_byte >> 4) & 0x0F];
        encoded_text[encoded_text_index++] = hexadecimal_digits[text_byte & 0x0F];
    }

    encoded_text[encoded_text_index] = '\0';
    return TALKSPHERE_SUCCESS;
}

static int write_all_bytes(
    int file_descriptor,
    const unsigned char *file_bytes,
    size_t file_byte_count
) {
    LOG_TRACE("write_all_bytes(): now we keep writing until every shared storage byte reaches disk");

    size_t written_file_byte_count = 0;
    while (written_file_byte_count < file_byte_count) {
        ssize_t written_byte_count = write(
            file_descriptor,
            file_bytes + written_file_byte_count,
            file_byte_count - written_file_byte_count
        );

        if (written_byte_count <= 0) {
            LOG_ERROR("Writing shared storage bytes failed so the stored file is incomplete");
            return TALKSPHERE_FAILURE;
        }

        written_file_byte_count += (size_t)written_byte_count;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_file_system_prepare(
    const char *app_storage_directory_path,
    char *storage_directory_path,
    size_t storage_directory_path_size,
    char *file_directory_path,
    size_t file_directory_path_size,
    char *database_file_path,
    size_t database_file_path_size
) {
    LOG_TRACE("shared_storage_file_system_prepare(): now we prepare directories and paths for shared storage");

    char resolved_app_storage_directory_path[PATH_MAX];
    if (resolve_app_storage_directory_path(
            app_storage_directory_path,
            resolved_app_storage_directory_path,
            sizeof(resolved_app_storage_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (create_directory_if_missing(resolved_app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (append_path_part(
            resolved_app_storage_directory_path,
            SHARED_STORAGE_DIRECTORY_NAME,
            storage_directory_path,
            storage_directory_path_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (create_directory_if_missing(storage_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (append_path_part(
            storage_directory_path,
            SHARED_STORAGE_FILE_DIRECTORY_NAME,
            file_directory_path,
            file_directory_path_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (create_directory_if_missing(file_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return append_path_part(
        storage_directory_path,
        SHARED_STORAGE_DATABASE_FILE_NAME,
        database_file_path,
        database_file_path_size
    );
}

int shared_storage_file_system_build_file_path(
    const char *file_directory_path,
    const char *shared_file_id,
    const char *owner_id,
    char *stored_file_path,
    size_t stored_file_path_size
) {
    LOG_TRACE("shared_storage_file_system_build_file_path(): now we build the stored file path from safe encoded identifiers");

    char encoded_owner_id[PATH_MAX];
    char encoded_shared_file_id[PATH_MAX];

    if (append_text_as_hex(
            owner_id,
            encoded_owner_id,
            sizeof(encoded_owner_id)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (append_text_as_hex(
            shared_file_id,
            encoded_shared_file_id,
            sizeof(encoded_shared_file_id)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (snprintf(
            stored_file_path,
            stored_file_path_size,
            "%s/%s_%s.bin",
            file_directory_path,
            encoded_owner_id,
            encoded_shared_file_id
        ) >= (int)stored_file_path_size
    ) {
        LOG_ERROR("The shared storage file path is too long");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_file_system_write_file(
    const char *stored_file_path,
    const unsigned char *file_bytes,
    size_t file_byte_count
) {
    LOG_TRACE("shared_storage_file_system_write_file(): now we save shared storage bytes into their owned file");

    int file_descriptor = open(
        stored_file_path,
        O_WRONLY | O_CREAT | O_TRUNC,
        0600
    );
    if (file_descriptor < 0) {
        LOG_ERROR("Opening a shared storage file for writing failed so the file cannot be stored");
        return TALKSPHERE_FAILURE;
    }

    int write_result = write_all_bytes(
        file_descriptor,
        file_bytes,
        file_byte_count
    );

    if (close(file_descriptor) != 0) {
        LOG_ERROR("Closing a written shared storage file failed so the stored bytes may not be durable");
        return TALKSPHERE_FAILURE;
    }

    return write_result;
}

int shared_storage_file_system_read_file(
    const char *stored_file_path,
    unsigned char *file_bytes,
    size_t file_byte_capacity,
    size_t expected_file_byte_count,
    size_t *recovered_file_byte_count
) {
    LOG_TRACE("shared_storage_file_system_read_file(): now we recover bytes from a shared storage file");

    if (expected_file_byte_count > file_byte_capacity) {
        LOG_WARN("The recovery buffer is too small so the shared storage file cannot be returned without truncation");
        return TALKSPHERE_FAILURE;
    }

    FILE *stored_file = fopen(
        stored_file_path,
        "rb"
    );
    if (stored_file == NULL) {
        LOG_ERROR("Opening a shared storage file for reading failed so the entry cannot be recovered");
        return TALKSPHERE_FAILURE;
    }

    size_t read_file_byte_count = fread(
        file_bytes,
        sizeof(unsigned char),
        expected_file_byte_count,
        stored_file
    );

    if (fclose(stored_file) != 0) {
        LOG_ERROR("Closing a shared storage file after reading failed");
        return TALKSPHERE_FAILURE;
    }

    if (read_file_byte_count != expected_file_byte_count) {
        LOG_ERROR("Reading a shared storage file returned fewer bytes than metadata promised");
        return TALKSPHERE_FAILURE;
    }

    *recovered_file_byte_count = read_file_byte_count;
    return TALKSPHERE_SUCCESS;
}

int shared_storage_file_system_delete_file(
    const char *stored_file_path
) {
    LOG_TRACE("shared_storage_file_system_delete_file(): now we remove a shared storage file from disk");

    if (unlink(stored_file_path) != 0) {
        if (errno == ENOENT) {
            LOG_WARN("The shared storage file was already missing while deleting metadata");
            return TALKSPHERE_SUCCESS;
        }

        LOG_ERROR("Deleting a shared storage file failed so metadata should not be removed yet");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}
