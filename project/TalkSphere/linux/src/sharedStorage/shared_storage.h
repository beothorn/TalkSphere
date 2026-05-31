#ifndef TALKSPHERE_SHARED_STORAGE_H
#define TALKSPHERE_SHARED_STORAGE_H

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
