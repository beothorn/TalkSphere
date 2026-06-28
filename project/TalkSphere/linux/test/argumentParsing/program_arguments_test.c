#include "argumentParsing/program_arguments.h"
#include "common/result.h"
#include "test_support.h"

#include <stddef.h>
#include <string.h>

#define TEST_RUN_ARGUMENT_COUNT 4
#define TEST_TWO_ARGUMENT_COUNT 2
#define TEST_THREE_ARGUMENT_COUNT 3
#define TEST_FOUR_ARGUMENT_COUNT 4
#define TEST_FIVE_ARGUMENT_COUNT 5
#define TEST_SIX_ARGUMENT_COUNT 6
#define TEST_SEVEN_ARGUMENT_COUNT 7
#define TEST_TALK_MESSAGE_TEXT_ARGUMENT_INDEX 5
#define EXPECTED_LISTEN_PORT 9001
#define EXPECTED_PEER_PORT 9002
#define EXPECTED_LOCAL_CLIENT_PORT 8899
#define EXPECTED_MESSAGE_TEXT "hello world"
#define EXPECTED_HOME_DIRECTORY_PATH "/tmp/talksphere-directory-home-test"
#define EXPECTED_CONFIG_KEY_TEXT "availability"
#define EXPECTED_CONFIG_VALUE_TEXT "alwaysOn"

static int assert_run_arguments(
    struct program_arguments *program_arguments,
    int expected_listen_port,
    int expected_peer_port,
    const char *expected_storage_directory_path
) {
    if (program_arguments->program_mode != PROGRAM_MODE_RUN_SERVER) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments->listen_port != expected_listen_port) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments->peer_port != expected_peer_port) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments->app_storage_directory_path != expected_storage_directory_path) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int parse_arguments_for_test(
    int argument_count,
    char *argument_values[],
    struct program_arguments *program_arguments
) {
    return parse_program_arguments(
        argument_count,
        argument_values,
        program_arguments
    );
}

static int test_run_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "run",
        "9001",
        "9002"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_RUN_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return assert_run_arguments(
        &program_arguments,
        EXPECTED_LISTEN_PORT,
        EXPECTED_PEER_PORT,
        NULL
    );
}

static int test_global_directory_home_run_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "--home",
        EXPECTED_HOME_DIRECTORY_PATH,
        "run",
        "9001",
        "9002"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_SIX_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return assert_run_arguments(
        &program_arguments,
        EXPECTED_LISTEN_PORT,
        EXPECTED_PEER_PORT,
        argument_values[2]
    );
}

static int test_dry_run_config_home_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "--dry-run",
        "config",
        "get",
        "home"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FIVE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (!program_arguments.dry_run_is_enabled) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_PRINT_HOME
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_dry_run_rejects_short_argument(void) {
    char *argument_values[] = {
        "talksphere",
        "d",
        "config",
        "get",
        "home"
    };
    struct program_arguments program_arguments;

    return parse_arguments_for_test(
        TEST_FIVE_ARGUMENT_COUNT,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_run_rejects_positional_home_argument(void) {
    char *argument_values[] = {
        "talksphere",
        "run",
        "9001",
        "9002",
        EXPECTED_HOME_DIRECTORY_PATH
    };
    struct program_arguments program_arguments;

    return parse_arguments_for_test(
        TEST_FIVE_ARGUMENT_COUNT,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_config_set_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "config",
        "set",
        "availability",
        "alwaysOn"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FIVE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.config_key_text != argument_values[3]
        || program_arguments.config_value_text != argument_values[4]
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (strcmp(
            program_arguments.config_key_text,
            EXPECTED_CONFIG_KEY_TEXT
        ) != 0
        || strcmp(
            program_arguments.config_value_text,
            EXPECTED_CONFIG_VALUE_TEXT
        ) != 0
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_SET_CONFIG_VALUE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_config_get_availability_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "config",
        "get",
        "availability"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FOUR_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.config_key_text != argument_values[3]) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_GET_CONFIG_VALUE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_global_directory_home_config_set_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "--home",
        EXPECTED_HOME_DIRECTORY_PATH,
        "config",
        "set",
        "availability",
        "alwaysOn"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_SEVEN_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.app_storage_directory_path != argument_values[2]) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_SET_CONFIG_VALUE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_config_add_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "config",
        "add",
        "reachableAt",
        "www.example.com:9999"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FIVE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.config_key_text != argument_values[3]
        || program_arguments.config_value_text != argument_values[4]
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_ADD_CONFIG_VALUE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_config_remove_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "config",
        "remove",
        "reachableAt",
        "www.example.com:9999"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FIVE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.config_key_text != argument_values[3]
        || program_arguments.config_value_text != argument_values[4]
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_REMOVE_CONFIG_VALUE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_config_set_rejects_missing_value(void) {
    char *argument_values[] = {
        "talksphere",
        "config",
        "set",
        "availability"
    };
    struct program_arguments program_arguments;

    return parse_arguments_for_test(
        TEST_FOUR_ARGUMENT_COUNT,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_global_directory_home_offerings_get_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "--home",
        EXPECTED_HOME_DIRECTORY_PATH,
        "offerings",
        "get"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FIVE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.app_storage_directory_path != argument_values[2]) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_PRINT_LOCAL_OFFERINGS
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_files_home_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "files",
        "home"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_THREE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_PRINT_HOME
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_encryption_help_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "encryption"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_TWO_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_PRINT_ENCRYPTION_HELP
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_encryption_create_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "encryption",
        "create"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_THREE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_CREATE_ENCRYPTION_KEYS
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_encryption_message_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "encryption",
        "sign_message",
        "hello"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FOUR_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.message_text != argument_values[3]) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_SIGN_MESSAGE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_ledger_summary_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "ledger",
        "credit_summary"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_THREE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_PRINT_LEDGER_SUMMARY
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_network_ping_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "network",
        "ping",
        "127.0.0.1:8513"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FOUR_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.network_address_text != argument_values[3]) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_PING_NETWORK_PEER
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_offerings_add_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "offerings",
        "add",
        "bread"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FOUR_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.offering_text != argument_values[3]) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_ADD_OFFERING
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_talk_offerings_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "talk",
        "-p",
        "8899",
        "offerings"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FIVE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.local_client_port != EXPECTED_LOCAL_CLIENT_PORT) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_TALK_PRINT_REMOTE_OFFERINGS
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_talk_help_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "talk",
        "h"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_THREE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_PRINT_TALK_HELP
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_talk_offerings_rejects_invalid_port(void) {
    char *argument_values[] = {
        "talksphere",
        "talk",
        "-p",
        "bad",
        "offerings"
    };
    struct program_arguments program_arguments;

    return parse_arguments_for_test(
        TEST_FIVE_ARGUMENT_COUNT,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_talk_message_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "talk",
        "-p",
        "8899",
        "message",
        "hello world"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_SIX_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.local_client_port != EXPECTED_LOCAL_CLIENT_PORT) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.message_text != argument_values[TEST_TALK_MESSAGE_TEXT_ARGUMENT_INDEX]) {
        return TALKSPHERE_FAILURE;
    }

    if (strcmp(
            program_arguments.message_text,
            EXPECTED_MESSAGE_TEXT
        ) != 0
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_TALK_SEND_MESSAGE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_share_remote_list_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "share",
        "remote",
        "ls"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FOUR_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_LIST_REMOTE_SHARED_STORAGE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_credit_add_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "credit",
        "add",
        "1",
        "1234"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FIVE_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.credit_count != 1) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.credit_code_text != argument_values[4]) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_ADD_CREDIT_WITHDRAW_CODE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_credit_remove_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "credit",
        "remove",
        "1234"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FOUR_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.credit_code_text != argument_values[3]) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_REMOVE_CREDIT_WITHDRAW_CODE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_credit_withadraw_list_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "credit",
        "withadraw",
        "list"
    };
    struct program_arguments program_arguments;

    if (parse_arguments_for_test(
            TEST_FOUR_ARGUMENT_COUNT,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.program_mode == PROGRAM_MODE_LIST_CREDIT_WITHDRAW_CODES
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_credit_add_rejects_invalid_count(void) {
    char *argument_values[] = {
        "talksphere",
        "credit",
        "add",
        "0",
        "1234"
    };
    struct program_arguments program_arguments;

    return parse_arguments_for_test(
        TEST_FIVE_ARGUMENT_COUNT,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_invalid_run_port(void) {
    char *argument_values[] = {
        "talksphere",
        "run",
        "bad",
        "9002"
    };
    struct program_arguments program_arguments;

    return parse_arguments_for_test(
        TEST_FOUR_ARGUMENT_COUNT,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_equal_ports(void) {
    char *argument_values[] = {
        "talksphere",
        "run",
        "9001",
        "9001"
    };
    struct program_arguments program_arguments;

    return parse_arguments_for_test(
        TEST_FOUR_ARGUMENT_COUNT,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_encryption_create_rejects_extra_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "encryption",
        "create",
        "extra"
    };
    struct program_arguments program_arguments;

    return parse_arguments_for_test(
        TEST_FOUR_ARGUMENT_COUNT,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

int main(void) {
    const struct test_case test_cases[] = {
        TEST_CASE(test_run_arguments),
        TEST_CASE(test_global_directory_home_run_arguments),
        TEST_CASE(test_dry_run_config_home_arguments),
        TEST_CASE(test_dry_run_rejects_short_argument),
        TEST_CASE(test_run_rejects_positional_home_argument),
        TEST_CASE(test_global_directory_home_offerings_get_arguments),
        TEST_CASE(test_config_set_arguments),
        TEST_CASE(test_config_get_availability_arguments),
        TEST_CASE(test_global_directory_home_config_set_arguments),
        TEST_CASE(test_config_add_arguments),
        TEST_CASE(test_config_remove_arguments),
        TEST_CASE(test_config_set_rejects_missing_value),
        TEST_CASE(test_files_home_arguments),
        TEST_CASE(test_encryption_help_arguments),
        TEST_CASE(test_encryption_create_arguments),
        TEST_CASE(test_encryption_message_arguments),
        TEST_CASE(test_ledger_summary_arguments),
        TEST_CASE(test_network_ping_arguments),
        TEST_CASE(test_offerings_add_arguments),
        TEST_CASE(test_share_remote_list_arguments),
        TEST_CASE(test_talk_help_arguments),
        TEST_CASE(test_talk_offerings_arguments),
        TEST_CASE(test_talk_offerings_rejects_invalid_port),
        TEST_CASE(test_talk_message_arguments),
        TEST_CASE(test_invalid_run_port),
        TEST_CASE(test_equal_ports),
        TEST_CASE(test_encryption_create_rejects_extra_arguments),
        TEST_CASE(test_credit_add_arguments),
        TEST_CASE(test_credit_remove_arguments),
        TEST_CASE(test_credit_add_rejects_invalid_count),
        TEST_CASE(test_credit_withadraw_list_arguments)
    };

    return run_test_cases(
        test_cases,
        sizeof(test_cases) / sizeof(test_cases[0])
    );
}
