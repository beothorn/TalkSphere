#include "shared_storage.h"

#include "../common/result.h"
#include "../logging.h"

#include <stddef.h>

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
