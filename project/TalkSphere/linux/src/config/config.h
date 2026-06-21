#ifndef TALKSPHERE_CONFIG_H
#define TALKSPHERE_CONFIG_H

#include <stddef.h>

int config_get_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    char *config_value_text,
    size_t config_value_text_size
);

int config_set_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

int config_add_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

int config_remove_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

#endif
