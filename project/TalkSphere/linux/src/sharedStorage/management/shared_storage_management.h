#ifndef TALKSPHERE_SHARED_STORAGE_MANAGEMENT_H
#define TALKSPHERE_SHARED_STORAGE_MANAGEMENT_H

#include "../shared_storage.h"

#include <stddef.h>

struct shared_storage_managed_entry {
    char stored_file_path[4096];
    size_t file_byte_count;
};

typedef int (*shared_storage_expired_entry_callback)(
    const char *shared_file_id,
    const char *owner_id,
    const char *stored_file_path,
    void *callback_context
);

int shared_storage_management_prepare(
    const char *database_file_path
);

int shared_storage_management_save_entry(
    const char *database_file_path,
    const char *shared_file_id,
    const char *owner_id,
    const char *stored_file_path,
    size_t file_byte_count,
    long long expiration_time_seconds
);

int shared_storage_management_find_entry(
    const char *database_file_path,
    const char *shared_file_id,
    const char *owner_id,
    struct shared_storage_managed_entry *managed_entry
);

int shared_storage_management_delete_entry(
    const char *database_file_path,
    const char *shared_file_id,
    const char *owner_id
);

int shared_storage_management_for_each_expired_entry(
    const char *database_file_path,
    long long current_time_seconds,
    shared_storage_expired_entry_callback expired_entry_callback,
    void *callback_context
);

int shared_storage_management_delete_expired_entries(
    const char *database_file_path,
    long long current_time_seconds
);

int shared_storage_management_query(
    const char *database_file_path,
    const char *sql_query,
    shared_storage_query_row_callback row_callback,
    void *callback_context
);

#endif
