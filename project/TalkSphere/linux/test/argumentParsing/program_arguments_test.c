#include "argumentParsing/program_arguments.h"
#include "common/result.h"

#include <stddef.h>
#include <string.h>

#define TEST_RUN_ARGUMENT_COUNT 5
#define TEST_DRY_RUN_ARGUMENT_COUNT 4
#define TEST_TWO_ARGUMENT_COUNT 2
#define TEST_THREE_ARGUMENT_COUNT 3
#define TEST_FOUR_ARGUMENT_COUNT 4
#define TEST_SIX_ARGUMENT_COUNT 6
#define TEST_TALK_MESSAGE_TEXT_ARGUMENT_INDEX 5
#define EXPECTED_LISTEN_PORT 9001
#define EXPECTED_PEER_PORT 9002
#define EXPECTED_LOCAL_CLIENT_PORT 8899
#define EXPECTED_MESSAGE_TEXT "hello world"

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
        "9002",
        "/tmp/talksphere-arguments-test"
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
        argument_values[4]
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
            TEST_RUN_ARGUMENT_COUNT,
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
            TEST_RUN_ARGUMENT_COUNT,
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
        TEST_RUN_ARGUMENT_COUNT,
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
    if (test_run_arguments() != TALKSPHERE_SUCCESS) {
        return 1;
    }

    if (test_dry_run_config_home_arguments() != TALKSPHERE_SUCCESS) {
        return 2;
    }

    if (test_files_home_arguments() != TALKSPHERE_SUCCESS) {
        return 3;
    }

    if (test_encryption_help_arguments() != TALKSPHERE_SUCCESS) {
        return 4;
    }

    if (test_encryption_create_arguments() != TALKSPHERE_SUCCESS) {
        return 5;
    }

    if (test_encryption_message_arguments() != TALKSPHERE_SUCCESS) {
        return 6;
    }

    if (test_ledger_summary_arguments() != TALKSPHERE_SUCCESS) {
        return 7;
    }

    if (test_network_ping_arguments() != TALKSPHERE_SUCCESS) {
        return 8;
    }

    if (test_offerings_add_arguments() != TALKSPHERE_SUCCESS) {
        return 9;
    }

    if (test_share_remote_list_arguments() != TALKSPHERE_SUCCESS) {
        return 10;
    }

    if (test_talk_offerings_arguments() != TALKSPHERE_SUCCESS) {
        return 11;
    }

    if (test_talk_offerings_rejects_invalid_port() != TALKSPHERE_SUCCESS) {
        return 12;
    }

    if (test_talk_message_arguments() != TALKSPHERE_SUCCESS) {
        return 13;
    }

    if (test_invalid_run_port() != TALKSPHERE_SUCCESS) {
        return 14;
    }

    if (test_equal_ports() != TALKSPHERE_SUCCESS) {
        return 15;
    }

    if (test_encryption_create_rejects_extra_arguments() != TALKSPHERE_SUCCESS) {
        return 16;
    }

    return 0;
}
