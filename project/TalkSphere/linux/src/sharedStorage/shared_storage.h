#ifndef TALKSPHERE_SHARED_STORAGE_H
#define TALKSPHERE_SHARED_STORAGE_H

#include <stddef.h>

typedef int (*shared_storage_query_row_callback)(
    int column_count,
    const char **column_names,
    const char **column_values,
    void *callback_context
);

int shared_storage_store_data(
    const char *app_storage_directory_path,
    const unsigned char *file_bytes,
    size_t file_byte_count,
    const char *shared_file_id,
    const char *owner_id,
    long long expiration_time_seconds
);

int shared_storage_recover_data(
    const char *app_storage_directory_path,
    const char *shared_file_id,
    const char *owner_id,
    unsigned char *file_bytes,
    size_t file_byte_capacity,
    size_t *recovered_file_byte_count
);

int shared_storage_delete_entry(
    const char *app_storage_directory_path,
    const char *shared_file_id,
    const char *owner_id
);

int shared_storage_clean_up_expired_entries(
    const char *app_storage_directory_path,
    long long current_time_seconds
);

int shared_storage_query_file_manager(
    const char *app_storage_directory_path,
    const char *sql_query,
    shared_storage_query_row_callback row_callback,
    void *callback_context
);

int shared_storage_share_available_storage(
    const char *app_storage_directory_path
);

int shared_storage_recover_sold_storage(
    const char *app_storage_directory_path
);

int shared_storage_clear_aged_storage(
    const char *app_storage_directory_path,
    int maximum_storage_age_seconds
);

#endif
