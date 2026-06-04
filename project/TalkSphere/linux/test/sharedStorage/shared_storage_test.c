#include "sharedStorage/shared_storage.h"
#include "common/result.h"

#include <stddef.h>

int main(void) {
    const char *app_storage_directory_path = "/tmp/talksphere-shared-storage-test";

    if (shared_storage_share_available_storage(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return 1;
    }

    if (shared_storage_recover_sold_storage(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return 2;
    }

    if (shared_storage_clear_aged_storage(
            app_storage_directory_path,
            60
        ) != TALKSPHERE_SUCCESS
    ) {
        return 3;
    }

    if (shared_storage_share_available_storage(NULL) != TALKSPHERE_FAILURE) {
        return 4;
    }

    if (shared_storage_recover_sold_storage(NULL) != TALKSPHERE_FAILURE) {
        return 5;
    }

    if (shared_storage_clear_aged_storage(
            app_storage_directory_path,
            -1
        ) != TALKSPHERE_FAILURE
    ) {
        return 6;
    }

    return 0;
}
