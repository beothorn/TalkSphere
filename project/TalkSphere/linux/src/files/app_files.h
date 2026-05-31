#ifndef TALKSPHERE_APP_FILES_H
#define TALKSPHERE_APP_FILES_H

int ensure_app_files(
    const char *app_storage_directory_path
);

int read_local_identifier(
    const char *app_storage_directory_path,
    char *identifier_text,
    int identifier_text_size
);

#endif
