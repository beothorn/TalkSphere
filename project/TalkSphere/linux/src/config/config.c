#include "config.h"

#include "../common/app_file_names.h"
#include "../common/result.h"
#include "../logging.h"

#include <limits.h>
#include <stdio.h>
#include <string.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

#define CONFIG_FILE_TEXT_SIZE 16384
#define CONFIG_LINE_TEXT_SIZE 1024
#define CONFIG_KEY_SEPARATOR_CHARACTER '='
#define CONFIG_LINE_SEPARATOR_CHARACTER '\n'
#define CONFIG_STRING_TERMINATOR '\0'
#define CONFIG_VALUE_OFFSET_FROM_SEPARATOR 1
#define CONFIG_AVAILABILITY_KEY_TEXT "availability"
#define CONFIG_REACHABLE_AT_KEY_TEXT "reachableAt"

static int text_is_equal(
    const char *first_text,
    const char *second_text
) {
    LOG_TRACE("text_is_equal(): now we compare two config words");

    return strcmp(
        first_text,
        second_text
    ) == 0;
}

static int build_config_file_path(
    const char *app_storage_directory_path,
    char *config_file_path,
    size_t config_file_path_size
) {
    LOG_TRACE("build_config_file_path(): now we compute where the user config file lives");

    if (snprintf(
            config_file_path,
            config_file_path_size,
            "%s/%s",
            app_storage_directory_path,
            TALKSPHERE_CONFIG_FILE_NAME
        ) >= (int)config_file_path_size
    ) {
        LOG_ERROR("The config file path is too long so the config command cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int read_config_text(
    const char *config_file_path,
    char *config_file_text,
    size_t config_file_text_size
) {
    LOG_TRACE("read_config_text(): now we load the whole config file into memory because this file is intentionally small");

    FILE *config_file = fopen(
        config_file_path,
        "r"
    );
    if (config_file == NULL) {
        LOG_ERROR("Opening the config file failed so the config command cannot continue");
        return TALKSPHERE_FAILURE;
    }

    size_t read_bytes_count = fread(
        config_file_text,
        sizeof(char),
        config_file_text_size - 1,
        config_file
    );

    if (ferror(config_file)) {
        fclose(config_file);
        LOG_ERROR("Reading the config file failed so the config command cannot continue");
        return TALKSPHERE_FAILURE;
    }

    fclose(config_file);
    config_file_text[read_bytes_count] = CONFIG_STRING_TERMINATOR;
    return TALKSPHERE_SUCCESS;
}

static int write_config_text(
    const char *config_file_path,
    const char *config_file_text
) {
    LOG_TRACE("write_config_text(): now we replace the config file with the updated config text");

    FILE *config_file = fopen(
        config_file_path,
        "w"
    );
    if (config_file == NULL) {
        LOG_ERROR("Opening the config file for writing failed so the config command cannot continue");
        return TALKSPHERE_FAILURE;
    }

    size_t config_file_text_length = strlen(config_file_text);
    size_t written_bytes_count = fwrite(
        config_file_text,
        sizeof(char),
        config_file_text_length,
        config_file
    );

    if (written_bytes_count != config_file_text_length) {
        fclose(config_file);
        LOG_ERROR("Writing the config file failed so the config command cannot continue");
        return TALKSPHERE_FAILURE;
    }

    if (fclose(config_file) != 0) {
        LOG_ERROR("Closing the config file failed so the config command cannot be trusted");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int config_line_matches(
    const char *config_line_text,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_line_matches(): now we check whether one config line represents the requested key and value");

    size_t config_key_text_length = strlen(config_key_text);
    if (strncmp(
            config_line_text,
            config_key_text,
            config_key_text_length
        ) != 0
    ) {
        return 0;
    }

    if (config_line_text[config_key_text_length] != CONFIG_KEY_SEPARATOR_CHARACTER) {
        return 0;
    }

    return text_is_equal(
        config_line_text + config_key_text_length + CONFIG_VALUE_OFFSET_FROM_SEPARATOR,
        config_value_text
    );
}

static int config_line_has_key(
    const char *config_line_text,
    const char *config_key_text
) {
    LOG_TRACE("config_line_has_key(): now we check whether one config line belongs to the requested key");

    size_t config_key_text_length = strlen(config_key_text);
    return strncmp(
        config_line_text,
        config_key_text,
        config_key_text_length
    ) == 0
        && config_line_text[config_key_text_length] == CONFIG_KEY_SEPARATOR_CHARACTER;
}

static int append_config_line(
    char *updated_config_text,
    size_t updated_config_text_size,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("append_config_line(): now we append one key-value line to the updated config text");

    size_t updated_config_text_length = strlen(updated_config_text);
    if (snprintf(
            updated_config_text + updated_config_text_length,
            updated_config_text_size - updated_config_text_length,
            "%s%c%s%c",
            config_key_text,
            CONFIG_KEY_SEPARATOR_CHARACTER,
            config_value_text,
            CONFIG_LINE_SEPARATOR_CHARACTER
        ) >= (int)(updated_config_text_size - updated_config_text_length)
    ) {
        LOG_ERROR("The config file would become too large so the config command cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int copy_config_line(
    char *updated_config_text,
    size_t updated_config_text_size,
    const char *config_line_text
) {
    LOG_TRACE("copy_config_line(): now we preserve one existing config line");

    size_t updated_config_text_length = strlen(updated_config_text);
    if (snprintf(
            updated_config_text + updated_config_text_length,
            updated_config_text_size - updated_config_text_length,
            "%s%c",
            config_line_text,
            CONFIG_LINE_SEPARATOR_CHARACTER
        ) >= (int)(updated_config_text_size - updated_config_text_length)
    ) {
        LOG_ERROR("The config file would become too large while copying existing values");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int update_config_text_for_set(
    const char *config_file_text,
    const char *config_key_text,
    const char *config_value_text,
    char *updated_config_text,
    size_t updated_config_text_size
) {
    LOG_TRACE("update_config_text_for_set(): now we replace old scalar values and keep unrelated config lines");

    updated_config_text[0] = CONFIG_STRING_TERMINATOR;

    char config_file_text_copy[CONFIG_FILE_TEXT_SIZE];
    if (snprintf(
            config_file_text_copy,
            sizeof(config_file_text_copy),
            "%s",
            config_file_text
        ) >= (int)sizeof(config_file_text_copy)
    ) {
        LOG_ERROR("The config file is too large for this command to edit safely");
        return TALKSPHERE_FAILURE;
    }

    char *config_line_text = strtok(
        config_file_text_copy,
        "\n"
    );
    while (config_line_text != NULL) {
        if (!config_line_has_key(
                config_line_text,
                config_key_text
            )
        ) {
            if (copy_config_line(
                    updated_config_text,
                    updated_config_text_size,
                    config_line_text
                ) != TALKSPHERE_SUCCESS
            ) {
                return TALKSPHERE_FAILURE;
            }
        }

        config_line_text = strtok(
            NULL,
            "\n"
        );
    }

    return append_config_line(
        updated_config_text,
        updated_config_text_size,
        config_key_text,
        config_value_text
    );
}

static int find_scalar_config_value(
    const char *config_file_text,
    const char *config_key_text,
    char *config_value_text,
    size_t config_value_text_size
) {
    LOG_TRACE("find_scalar_config_value(): now we scan config lines for the requested scalar key");

    char config_file_text_copy[CONFIG_FILE_TEXT_SIZE];
    if (snprintf(
            config_file_text_copy,
            sizeof(config_file_text_copy),
            "%s",
            config_file_text
        ) >= (int)sizeof(config_file_text_copy)
    ) {
        LOG_ERROR("The config file is too large for this command to read safely");
        return TALKSPHERE_FAILURE;
    }

    char *config_line_text = strtok(
        config_file_text_copy,
        "\n"
    );
    while (config_line_text != NULL) {
        if (config_line_has_key(
                config_line_text,
                config_key_text
            )
        ) {
            size_t config_key_text_length = strlen(config_key_text);
            const char *found_config_value_text =
                config_line_text + config_key_text_length + CONFIG_VALUE_OFFSET_FROM_SEPARATOR;

            if (snprintf(
                    config_value_text,
                    config_value_text_size,
                    "%s",
                    found_config_value_text
                ) >= (int)config_value_text_size
            ) {
                LOG_ERROR("The config value is too long for the output buffer");
                return TALKSPHERE_FAILURE;
            }

            return TALKSPHERE_SUCCESS;
        }

        config_line_text = strtok(
            NULL,
            "\n"
        );
    }

    LOG_WARN("The config value is unwanted because it has not been configured yet");
    fprintf(
        stderr,
        "Config %s is not set\n",
        config_key_text
    );
    return TALKSPHERE_FAILURE;
}

static int update_config_text_for_add(
    const char *config_file_text,
    const char *config_key_text,
    const char *config_value_text,
    char *updated_config_text,
    size_t updated_config_text_size
) {
    LOG_TRACE("update_config_text_for_add(): now we add a list value when the exact line is not already present");

    updated_config_text[0] = CONFIG_STRING_TERMINATOR;

    char config_file_text_copy[CONFIG_FILE_TEXT_SIZE];
    if (snprintf(
            config_file_text_copy,
            sizeof(config_file_text_copy),
            "%s",
            config_file_text
        ) >= (int)sizeof(config_file_text_copy)
    ) {
        LOG_ERROR("The config file is too large for this command to edit safely");
        return TALKSPHERE_FAILURE;
    }

    int existing_value_was_found = 0;
    char *config_line_text = strtok(
        config_file_text_copy,
        "\n"
    );
    while (config_line_text != NULL) {
        if (config_line_matches(
                config_line_text,
                config_key_text,
                config_value_text
            )
        ) {
            existing_value_was_found = 1;
        }

        if (copy_config_line(
                updated_config_text,
                updated_config_text_size,
                config_line_text
            ) != TALKSPHERE_SUCCESS
        ) {
            return TALKSPHERE_FAILURE;
        }

        config_line_text = strtok(
            NULL,
            "\n"
        );
    }

    if (existing_value_was_found) {
        LOG_WARN("The config value is unwanted because it already exists");
        fprintf(
            stderr,
            "Config %s already contains %s\n",
            config_key_text,
            config_value_text
        );
        return TALKSPHERE_FAILURE;
    }

    return append_config_line(
        updated_config_text,
        updated_config_text_size,
        config_key_text,
        config_value_text
    );
}

static int update_config_text_for_remove(
    const char *config_file_text,
    const char *config_key_text,
    const char *config_value_text,
    char *updated_config_text,
    size_t updated_config_text_size
) {
    LOG_TRACE("update_config_text_for_remove(): now we remove a list value and preserve every other config line");

    updated_config_text[0] = CONFIG_STRING_TERMINATOR;

    char config_file_text_copy[CONFIG_FILE_TEXT_SIZE];
    if (snprintf(
            config_file_text_copy,
            sizeof(config_file_text_copy),
            "%s",
            config_file_text
        ) >= (int)sizeof(config_file_text_copy)
    ) {
        LOG_ERROR("The config file is too large for this command to edit safely");
        return TALKSPHERE_FAILURE;
    }

    int removed_value_was_found = 0;
    char *config_line_text = strtok(
        config_file_text_copy,
        "\n"
    );
    while (config_line_text != NULL) {
        if (config_line_matches(
                config_line_text,
                config_key_text,
                config_value_text
            )
        ) {
            removed_value_was_found = 1;
        } else if (copy_config_line(
                updated_config_text,
                updated_config_text_size,
                config_line_text
            ) != TALKSPHERE_SUCCESS
        ) {
            return TALKSPHERE_FAILURE;
        }

        config_line_text = strtok(
            NULL,
            "\n"
        );
    }

    if (!removed_value_was_found) {
        LOG_WARN("The config value is unwanted because it does not exist");
        fprintf(
            stderr,
            "Config %s does not contain %s\n",
            config_key_text,
            config_value_text
        );
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int update_config_file(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text,
    int (*update_config_text)(
        const char *config_file_text,
        const char *config_key_text,
        const char *config_value_text,
        char *updated_config_text,
        size_t updated_config_text_size
    )
) {
    LOG_TRACE("update_config_file(): now we load, transform, and store the config file");
    LOG_DEBUG(
        "Updating config key %s with value %s",
        config_key_text,
        config_value_text
    );

    char config_file_path[PATH_MAX];
    if (build_config_file_path(
            app_storage_directory_path,
            config_file_path,
            sizeof(config_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char config_file_text[CONFIG_FILE_TEXT_SIZE];
    if (read_config_text(
            config_file_path,
            config_file_text,
            sizeof(config_file_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char updated_config_text[CONFIG_FILE_TEXT_SIZE];
    if (update_config_text(
            config_file_text,
            config_key_text,
            config_value_text,
            updated_config_text,
            sizeof(updated_config_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return write_config_text(
        config_file_path,
        updated_config_text
    );
}

static int validate_scalar_config_key(
    const char *config_key_text
) {
    LOG_TRACE("validate_scalar_config_key(): now we check whether this config key supports replacement");

    if (text_is_equal(
            config_key_text,
            CONFIG_AVAILABILITY_KEY_TEXT
        )
    ) {
        return TALKSPHERE_SUCCESS;
    }

    if (text_is_equal(
            config_key_text,
            CONFIG_REACHABLE_AT_KEY_TEXT
        )
    ) {
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("The config key is unwanted because it cannot be replaced with set");
    fprintf(
        stderr,
        "Unsupported config set key: %s\n",
        config_key_text
    );
    return TALKSPHERE_FAILURE;
}

static int validate_list_config_key(
    const char *config_key_text
) {
    LOG_TRACE("validate_list_config_key(): now we check whether this config key supports add and remove");

    if (text_is_equal(
            config_key_text,
            CONFIG_REACHABLE_AT_KEY_TEXT
        )
    ) {
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("The config key is unwanted because it is not a list config key");
    fprintf(
        stderr,
        "Unsupported config list key: %s\n",
        config_key_text
    );
    return TALKSPHERE_FAILURE;
}

static int validate_config_value(
    const char *config_value_text
) {
    LOG_TRACE("validate_config_value(): now we check that a config value fits the line-oriented config file");

    if (config_value_text[0] == CONFIG_STRING_TERMINATOR
        || strchr(
            config_value_text,
            CONFIG_LINE_SEPARATOR_CHARACTER
        ) != NULL
        || strlen(config_value_text) >= CONFIG_LINE_TEXT_SIZE
    ) {
        LOG_WARN("The config value is unwanted because it is empty, multiline, or too long for one config line");
        fprintf(
            stderr,
            "Invalid config value: %s\n",
            config_value_text
        );
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int config_set_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_set_value(): now we set a scalar config value");

    if (validate_scalar_config_key(config_key_text) != TALKSPHERE_SUCCESS
        || validate_config_value(config_value_text) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return update_config_file(
        app_storage_directory_path,
        config_key_text,
        config_value_text,
        update_config_text_for_set
    );
}

int config_get_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    char *config_value_text,
    size_t config_value_text_size
) {
    LOG_TRACE("config_get_value(): now we read a scalar config value");

    if (validate_scalar_config_key(config_key_text) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    char config_file_path[PATH_MAX];
    if (build_config_file_path(
            app_storage_directory_path,
            config_file_path,
            sizeof(config_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char config_file_text[CONFIG_FILE_TEXT_SIZE];
    if (read_config_text(
            config_file_path,
            config_file_text,
            sizeof(config_file_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return find_scalar_config_value(
        config_file_text,
        config_key_text,
        config_value_text,
        config_value_text_size
    );
}

int config_add_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_add_value(): now we add one value to a list config key");

    if (validate_list_config_key(config_key_text) != TALKSPHERE_SUCCESS
        || validate_config_value(config_value_text) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return update_config_file(
        app_storage_directory_path,
        config_key_text,
        config_value_text,
        update_config_text_for_add
    );
}

int config_remove_value(
    const char *app_storage_directory_path,
    const char *config_key_text,
    const char *config_value_text
) {
    LOG_TRACE("config_remove_value(): now we remove one value from a list config key");

    if (validate_list_config_key(config_key_text) != TALKSPHERE_SUCCESS
        || validate_config_value(config_value_text) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return update_config_file(
        app_storage_directory_path,
        config_key_text,
        config_value_text,
        update_config_text_for_remove
    );
}
