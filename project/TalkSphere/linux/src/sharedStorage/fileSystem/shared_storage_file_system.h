#ifndef TALKSPHERE_SHARED_STORAGE_FILE_SYSTEM_H
#define TALKSPHERE_SHARED_STORAGE_FILE_SYSTEM_H

#include <stddef.h>

int shared_storage_file_system_prepare(
    const char *app_storage_directory_path,
    char *storage_directory_path,
    size_t storage_directory_path_size,
    char *file_directory_path,
    size_t file_directory_path_size,
    char *database_file_path,
    size_t database_file_path_size
);

int shared_storage_file_system_build_file_path(
    const char *file_directory_path,
    const char *shared_file_id,
    const char *owner_id,
    char *stored_file_path,
    size_t stored_file_path_size
);

int shared_storage_file_system_write_file(
    const char *stored_file_path,
    const unsigned char *file_bytes,
    size_t file_byte_count
);

int shared_storage_file_system_read_file(
    const char *stored_file_path,
    unsigned char *file_bytes,
    size_t file_byte_capacity,
    size_t expected_file_byte_count,
    size_t *recovered_file_byte_count
);

int shared_storage_file_system_delete_file(
    const char *stored_file_path
);

#endif
