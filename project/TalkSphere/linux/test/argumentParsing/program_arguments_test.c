#include "argumentParsing/program_arguments.h"
#include "common/result.h"

#include <stddef.h>

static int assert_server_arguments(
    struct program_arguments *program_arguments,
    int expected_client_port,
    int expected_server_port,
    const char *expected_storage_directory_path
) {
    if (program_arguments->program_mode != PROGRAM_MODE_RUN_SERVER) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments->client_port != expected_client_port) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments->server_port != expected_server_port) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments->app_storage_directory_path != expected_storage_directory_path) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int test_default_server_arguments(void) {
    char *argument_values[] = {
        "talksphere"
    };
    struct program_arguments program_arguments;

    if (parse_program_arguments(
            1,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return assert_server_arguments(
        &program_arguments,
        DEFAULT_CLIENT_PORT,
        DEFAULT_SERVER_PORT,
        NULL
    );
}

static int test_custom_server_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "9001",
        "9002",
        "/tmp/talksphere-arguments-test"
    };
    struct program_arguments program_arguments;

    if (parse_program_arguments(
            4,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return assert_server_arguments(
        &program_arguments,
        9001,
        9002,
        argument_values[3]
    );
}

static int test_ledger_summary_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "--ledger-summary",
        "/tmp/talksphere-ledger-summary-test"
    };
    struct program_arguments program_arguments;

    if (parse_program_arguments(
            3,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.program_mode != PROGRAM_MODE_PRINT_LEDGER_SUMMARY) {
        return TALKSPHERE_FAILURE;
    }

    return program_arguments.app_storage_directory_path == argument_values[2]
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_invalid_client_port(void) {
    char *argument_values[] = {
        "talksphere",
        "bad",
        "9002"
    };
    struct program_arguments program_arguments;

    return parse_program_arguments(
        3,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_invalid_server_port(void) {
    char *argument_values[] = {
        "talksphere",
        "9001",
        "bad"
    };
    struct program_arguments program_arguments;

    return parse_program_arguments(
        3,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_equal_ports(void) {
    char *argument_values[] = {
        "talksphere",
        "9001",
        "9001"
    };
    struct program_arguments program_arguments;

    return parse_program_arguments(
        3,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

static int test_ledger_summary_rejects_extra_arguments(void) {
    char *argument_values[] = {
        "talksphere",
        "--ledger-summary",
        "/tmp/talksphere-ledger-summary-test",
        "extra"
    };
    struct program_arguments program_arguments;

    return parse_program_arguments(
        4,
        argument_values,
        &program_arguments
    ) == TALKSPHERE_FAILURE
        ? TALKSPHERE_SUCCESS
        : TALKSPHERE_FAILURE;
}

int main(void) {
    if (test_default_server_arguments() != TALKSPHERE_SUCCESS) {
        return 1;
    }

    if (test_custom_server_arguments() != TALKSPHERE_SUCCESS) {
        return 2;
    }

    if (test_ledger_summary_arguments() != TALKSPHERE_SUCCESS) {
        return 3;
    }

    if (test_invalid_client_port() != TALKSPHERE_SUCCESS) {
        return 4;
    }

    if (test_invalid_server_port() != TALKSPHERE_SUCCESS) {
        return 5;
    }

    if (test_equal_ports() != TALKSPHERE_SUCCESS) {
        return 6;
    }

    if (test_ledger_summary_rejects_extra_arguments() != TALKSPHERE_SUCCESS) {
        return 7;
    }

    return 0;
}
