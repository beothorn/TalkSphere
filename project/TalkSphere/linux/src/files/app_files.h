#ifndef TALKSPHERE_APP_FILES_H
#define TALKSPHERE_APP_FILES_H

#include <stddef.h>

int resolve_app_storage_directory_path(
    const char *app_storage_directory_path,
    char *resolved_directory_path,
    size_t resolved_directory_path_size
);

int ensure_app_files(
    const char *app_storage_directory_path
);

int read_local_identifier(
    const char *app_storage_directory_path,
    char *identifier_text,
    int identifier_text_size
);

#endif
