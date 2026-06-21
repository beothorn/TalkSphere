#ifndef TALKSPHERE_CONFIG_APPLICATION_H
#define TALKSPHERE_CONFIG_APPLICATION_H

int config_application_print_get_dry_run(
    const char *app_storage_directory_path,
    const char *config_key_text
);

int config_application_print_set_dry_run(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

int config_application_print_add_dry_run(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

int config_application_print_remove_dry_run(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

int config_application_set_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

int config_application_get_value(
    const char *app_storage_directory_path,
    const char *config_key_text
);

int config_application_add_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

int config_application_remove_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
);

#endif
