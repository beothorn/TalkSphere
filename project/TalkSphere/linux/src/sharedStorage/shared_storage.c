#include "shared_storage.h"

#include "../common/result.h"
#include "../logging.h"
#include "fileSystem/shared_storage_file_system.h"
#include "management/shared_storage_management.h"

#include <limits.h>
#include <stddef.h>
#include <string.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

struct expired_entry_cleanup_context {
    int deleted_file_count;
};

static int shared_storage_prepare_paths(
    const char *app_storage_directory_path,
    char *file_directory_path,
    size_t file_directory_path_size,
    char *database_file_path,
    size_t database_file_path_size
) {
    LOG_TRACE("shared_storage_prepare_paths(): now we prepare shared storage filesystem and metadata paths");

    char storage_directory_path[PATH_MAX];
    return shared_storage_file_system_prepare(
        app_storage_directory_path,
        storage_directory_path,
        sizeof(storage_directory_path),
        file_directory_path,
        file_directory_path_size,
        database_file_path,
        database_file_path_size
    );
}

static int shared_storage_validate_required_text(
    const char *text_value,
    const char *value_name
) {
    LOG_TRACE("shared_storage_validate_required_text(): now we validate required shared storage text");

    if (text_value == NULL || text_value[0] == '\0') {
        LOG_WARN("A required shared storage text value is missing so the file manager cannot identify the entry");
        LOG_DEBUG("Missing shared storage text value named %s", value_name);
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int shared_storage_validate_entry_identity(
    const char *shared_file_id,
    const char *owner_id
) {
    LOG_TRACE("shared_storage_validate_entry_identity(): now we validate the shared file id and owner id");

    if (shared_storage_validate_required_text(
            shared_file_id,
            "shared_file_id"
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return shared_storage_validate_required_text(
        owner_id,
        "owner_id"
    );
}

static int delete_expired_entry(
    const char *shared_file_id,
    const char *owner_id,
    const char *stored_file_path,
    void *callback_context
) {
    LOG_TRACE("delete_expired_entry(): now we delete one expired shared storage file and its metadata");

    struct expired_entry_cleanup_context *cleanup_context = callback_context;

    if (shared_storage_file_system_delete_file(stored_file_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    cleanup_context->deleted_file_count++;
    LOG_DEBUG("Deleted expired shared storage file before metadata cleanup for id %s and owner %s", shared_file_id, owner_id);
    return TALKSPHERE_SUCCESS;
}

int shared_storage_store_data(
    const char *app_storage_directory_path,
    const unsigned char *file_bytes,
    size_t file_byte_count,
    const char *shared_file_id,
    const char *owner_id,
    long long expiration_time_seconds
) {
    LOG_TRACE("shared_storage_store_data(): now we store bytes and metadata for a shared storage entry");
    LOG_DEBUG("Storing shared storage data with byte count %zu", file_byte_count);

    if (file_bytes == NULL && file_byte_count > 0) {
        LOG_WARN("Shared storage bytes are missing even though the caller requested bytes to be stored");
        return TALKSPHERE_FAILURE;
    }

    if (shared_storage_validate_entry_identity(
            shared_file_id,
            owner_id
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char file_directory_path[PATH_MAX];
    char database_file_path[PATH_MAX];
    if (shared_storage_prepare_paths(
            app_storage_directory_path,
            file_directory_path,
            sizeof(file_directory_path),
            database_file_path,
            sizeof(database_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (shared_storage_management_prepare(database_file_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    char stored_file_path[PATH_MAX];
    if (shared_storage_file_system_build_file_path(
            file_directory_path,
            shared_file_id,
            owner_id,
            stored_file_path,
            sizeof(stored_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (shared_storage_file_system_write_file(
            stored_file_path,
            file_bytes,
            file_byte_count
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (shared_storage_management_save_entry(
            database_file_path,
            shared_file_id,
            owner_id,
            stored_file_path,
            file_byte_count,
            expiration_time_seconds
        ) != TALKSPHERE_SUCCESS
    ) {
        shared_storage_file_system_delete_file(stored_file_path);
        return TALKSPHERE_FAILURE;
    }

    LOG_INFO("Shared storage data was saved with metadata");
    return TALKSPHERE_SUCCESS;
}

int shared_storage_recover_data(
    const char *app_storage_directory_path,
    const char *shared_file_id,
    const char *owner_id,
    unsigned char *file_bytes,
    size_t file_byte_capacity,
    size_t *recovered_file_byte_count
) {
    LOG_TRACE("shared_storage_recover_data(): now we recover a shared storage entry by id and owner");

    if (file_bytes == NULL || recovered_file_byte_count == NULL) {
        LOG_WARN("Shared storage recovery needs an output buffer and a recovered byte count pointer");
        return TALKSPHERE_FAILURE;
    }

    if (shared_storage_validate_entry_identity(
            shared_file_id,
            owner_id
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char file_directory_path[PATH_MAX];
    char database_file_path[PATH_MAX];
    if (shared_storage_prepare_paths(
            app_storage_directory_path,
            file_directory_path,
            sizeof(file_directory_path),
            database_file_path,
            sizeof(database_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    (void)file_directory_path;

    if (shared_storage_management_prepare(database_file_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    struct shared_storage_managed_entry managed_entry;
    if (shared_storage_management_find_entry(
            database_file_path,
            shared_file_id,
            owner_id,
            &managed_entry
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return shared_storage_file_system_read_file(
        managed_entry.stored_file_path,
        file_bytes,
        file_byte_capacity,
        managed_entry.file_byte_count,
        recovered_file_byte_count
    );
}

int shared_storage_delete_entry(
    const char *app_storage_directory_path,
    const char *shared_file_id,
    const char *owner_id
) {
    LOG_TRACE("shared_storage_delete_entry(): now we force delete one shared storage entry");

    if (shared_storage_validate_entry_identity(
            shared_file_id,
            owner_id
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char file_directory_path[PATH_MAX];
    char database_file_path[PATH_MAX];
    if (shared_storage_prepare_paths(
            app_storage_directory_path,
            file_directory_path,
            sizeof(file_directory_path),
            database_file_path,
            sizeof(database_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    (void)file_directory_path;

    if (shared_storage_management_prepare(database_file_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    struct shared_storage_managed_entry managed_entry;
    if (shared_storage_management_find_entry(
            database_file_path,
            shared_file_id,
            owner_id,
            &managed_entry
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (shared_storage_file_system_delete_file(managed_entry.stored_file_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return shared_storage_management_delete_entry(
        database_file_path,
        shared_file_id,
        owner_id
    );
}

int shared_storage_clean_up_expired_entries(
    const char *app_storage_directory_path,
    long long current_time_seconds
) {
    LOG_TRACE("shared_storage_clean_up_expired_entries(): now we delete every shared storage entry that has expired");

    char file_directory_path[PATH_MAX];
    char database_file_path[PATH_MAX];
    if (shared_storage_prepare_paths(
            app_storage_directory_path,
            file_directory_path,
            sizeof(file_directory_path),
            database_file_path,
            sizeof(database_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    (void)file_directory_path;

    if (shared_storage_management_prepare(database_file_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    struct expired_entry_cleanup_context cleanup_context = {
        .deleted_file_count = 0
    };

    if (shared_storage_management_for_each_expired_entry(
            database_file_path,
            current_time_seconds,
            delete_expired_entry,
            &cleanup_context
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    LOG_DEBUG("Deleted %d expired shared storage files before removing metadata", cleanup_context.deleted_file_count);
    return shared_storage_management_delete_expired_entries(
        database_file_path,
        current_time_seconds
    );
}

int shared_storage_query_file_manager(
    const char *app_storage_directory_path,
    const char *sql_query,
    shared_storage_query_row_callback row_callback,
    void *callback_context
) {
    LOG_TRACE("shared_storage_query_file_manager(): now we query shared storage file manager metadata");

    if (sql_query == NULL || sql_query[0] == '\0' || row_callback == NULL) {
        LOG_WARN("Shared storage query needs SQL text and a row callback");
        return TALKSPHERE_FAILURE;
    }

    char file_directory_path[PATH_MAX];
    char database_file_path[PATH_MAX];
    if (shared_storage_prepare_paths(
            app_storage_directory_path,
            file_directory_path,
            sizeof(file_directory_path),
            database_file_path,
            sizeof(database_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    (void)file_directory_path;

    if (shared_storage_management_prepare(database_file_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return shared_storage_management_query(
        database_file_path,
        sql_query,
        row_callback,
        callback_context
    );
}

static int shared_storage_validate_app_storage_directory_path(
    const char *app_storage_directory_path
) {
    LOG_TRACE("shared_storage_validate_app_storage_directory_path(): now we confirm shared storage has a storage root before placeholder work starts");

    if (app_storage_directory_path == NULL) {
        LOG_WARN("Shared storage path is unwanted because storage sharing needs an app storage root to keep its files separate");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_share_available_storage(
    const char *app_storage_directory_path
) {
    LOG_TRACE("shared_storage_share_available_storage(): now we enter the placeholder that will publish storage available for peers");

    if (shared_storage_validate_app_storage_directory_path(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_recover_sold_storage(
    const char *app_storage_directory_path
) {
    LOG_TRACE("shared_storage_recover_sold_storage(): now we enter the placeholder that will recover storage previously sold to peers");

    if (shared_storage_validate_app_storage_directory_path(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int shared_storage_clear_aged_storage(
    const char *app_storage_directory_path,
    int maximum_storage_age_seconds
) {
    LOG_TRACE("shared_storage_clear_aged_storage(): now we enter the placeholder that will clear shared storage after its maximum age passes");

    if (shared_storage_validate_app_storage_directory_path(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (maximum_storage_age_seconds < 0) {
        LOG_WARN("Shared storage age is unwanted because a negative age would make cleanup delete data before time can pass");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}
