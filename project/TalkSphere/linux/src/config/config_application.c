#include "config_application.h"

#include "config.h"
#include "../common/result.h"
#include "../logging.h"

#include <stdio.h>

#define CONFIG_VALUE_TEXT_SIZE 1024

int config_application_print_get_dry_run(
    const char *app_storage_directory_path,
    const char *config_key_text
) {
    LOG_TRACE("config_application_print_get_dry_run(): now we describe the config get operation without reading files");

    printf(
        "Would get config %s from %s\n",
        config_key_text,
        app_storage_directory_path
    );

    return TALKSPHERE_SUCCESS;
}

int config_application_print_set_dry_run(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_application_print_set_dry_run(): now we describe the config set operation without writing files");

    printf(
        "Would set config %s to %s in %s\n",
        config_key_text,
        config_value_text,
        app_storage_directory_path
    );

    return TALKSPHERE_SUCCESS;
}

int config_application_print_add_dry_run(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_application_print_add_dry_run(): now we describe the config add operation without writing files");

    printf(
        "Would add config %s value %s in %s\n",
        config_key_text,
        config_value_text,
        app_storage_directory_path
    );

    return TALKSPHERE_SUCCESS;
}

int config_application_print_remove_dry_run(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_application_print_remove_dry_run(): now we describe the config remove operation without writing files");

    printf(
        "Would remove config %s value %s in %s\n",
        config_key_text,
        config_value_text,
        app_storage_directory_path
    );

    return TALKSPHERE_SUCCESS;
}

int config_application_set_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_application_set_value(): now we delegate config replacement to the config module");

    if (config_set_value(
            app_storage_directory_path,
            config_key_text,
            config_value_text
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Config %s set to %s\n",
        config_key_text,
        config_value_text
    );

    return TALKSPHERE_SUCCESS;
}

int config_application_get_value(
    const char *app_storage_directory_path,
    const char *config_key_text
) {
    LOG_TRACE("config_application_get_value(): now we read and print a config value");

    char config_value_text[CONFIG_VALUE_TEXT_SIZE];
    if (config_get_value(
            app_storage_directory_path,
            config_key_text,
            config_value_text,
            sizeof(config_value_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "%s\n",
        config_value_text
    );

    return TALKSPHERE_SUCCESS;
}

int config_application_add_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_application_add_value(): now we delegate config list insertion to the config module");

    if (config_add_value(
            app_storage_directory_path,
            config_key_text,
            config_value_text
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Config %s added %s\n",
        config_key_text,
        config_value_text
    );

    return TALKSPHERE_SUCCESS;
}

int config_application_remove_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_application_remove_value(): now we delegate config list removal to the config module");

    if (config_remove_value(
            app_storage_directory_path,
            config_key_text,
            config_value_text
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Config %s removed %s\n",
        config_key_text,
        config_value_text
    );

    return TALKSPHERE_SUCCESS;
}
