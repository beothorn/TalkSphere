#include "offerings.h"

#include "../common/app_file_names.h"
#include "../common/result.h"
#include "../logging.h"

#include <limits.h>
#include <stdio.h>
#include <string.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

static int build_offerings_file_path(
    const char *app_storage_directory_path,
    char *offerings_file_path,
    size_t offerings_file_path_size
) {
    LOG_TRACE("build_offerings_file_path(): now we compute the offerings file path from the storage directory");

    if (app_storage_directory_path == NULL) {
        LOG_WARN("Offerings path is unwanted because reading local offerings needs an app storage root");
        return TALKSPHERE_FAILURE;
    }

    if (snprintf(
            offerings_file_path,
            offerings_file_path_size,
            "%s/%s",
            app_storage_directory_path,
            TALKSPHERE_OFFERINGS_FILE_NAME
        ) >= (int)offerings_file_path_size
    ) {
        LOG_ERROR("The offerings file path is too long so the local offerings cannot be read");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int read_offerings_file(
    const char *offerings_file_path,
    char *offerings_text,
    size_t offerings_text_size
) {
    LOG_TRACE("read_offerings_file(): now we read the local offerings document as an opaque protocol text");

    FILE *offerings_file = fopen(
        offerings_file_path,
        "r"
    );
    if (offerings_file == NULL) {
        LOG_WARN("The offerings file is missing or unavailable so peers cannot learn what this entity buys or sells");
        return TALKSPHERE_FAILURE;
    }

    size_t read_bytes_count = fread(
        offerings_text,
        sizeof(char),
        offerings_text_size - 1,
        offerings_file
    );

    if (ferror(offerings_file)) {
        fclose(offerings_file);
        LOG_ERROR("Reading the offerings file failed so peers cannot receive the local offerings document");
        return TALKSPHERE_FAILURE;
    }

    int next_file_character = fgetc(offerings_file);
    if (next_file_character != EOF) {
        fclose(offerings_file);
        LOG_WARN("The offerings buffer is too small so returning partial offerings would mislead peers");
        return TALKSPHERE_FAILURE;
    }

    if (ferror(offerings_file)) {
        fclose(offerings_file);
        LOG_ERROR("Checking the offerings file length failed so peers cannot receive the local offerings document");
        return TALKSPHERE_FAILURE;
    }

    fclose(offerings_file);
    offerings_text[read_bytes_count] = '\0';
    return TALKSPHERE_SUCCESS;
}

int read_local_offerings(
    const char *app_storage_directory_path,
    char *offerings_text,
    size_t offerings_text_size
) {
    LOG_TRACE("read_local_offerings(): now we load what this entity wants to buy and sell from the file system");

    if (offerings_text == NULL) {
        LOG_WARN("Offerings output is unwanted because the caller gave nowhere to return the document");
        return TALKSPHERE_FAILURE;
    }

    if (offerings_text_size == 0) {
        LOG_WARN("Offerings output size is unwanted because even the string terminator would not fit");
        return TALKSPHERE_FAILURE;
    }

    char offerings_file_path[PATH_MAX];
    if (build_offerings_file_path(
            app_storage_directory_path,
            offerings_file_path,
            sizeof(offerings_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return read_offerings_file(
        offerings_file_path,
        offerings_text,
        offerings_text_size
    );
}
