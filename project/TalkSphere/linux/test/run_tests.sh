#!/usr/bin/env bash
set -u

test_build_directory="build/test"
failure_count=0

all_test_paths=(
    "test/argumentParsing/program_arguments_test.c"
    "test/config/config_test.c"
    "test/creditWithdraw/credit_withdraw_test.c"
    "test/argumentParsing/id_and_help_command_test.sh"
    "test/encryption/encryption_test.c"
    "test/files/app_files_test.c"
    "test/ledger/ledger_summary_test.sh"
    "test/network/socket_channel_test.sh"
    "test/offerings/offerings_test.c"
    "test/sharedStorage/shared_storage_test.c"
)

normalize_test_path() {
    local requested_test_path="$1"

    if [[ "$requested_test_path" == test/* ]]; then
        printf '%s\n' "$requested_test_path"
        return
    fi

    printf 'test/%s\n' "$requested_test_path"
}

test_binary_path_for() {
    local test_source_path="$1"
    local test_binary_name

    test_binary_name="${test_source_path#test/}"
    test_binary_name="${test_binary_name%.c}"
    test_binary_name="${test_binary_name//\//_}"

    printf '%s/%s\n' "$test_build_directory" "$test_binary_name"
}

source_files_for() {
    local test_source_path="$1"

    case "$test_source_path" in
        test/argumentParsing/program_arguments_test.c)
            printf '%s\n' "src/argumentParsing/program_arguments.c"
            ;;
        test/encryption/encryption_test.c)
            printf '%s\n' "src/encryption/encryption.c"
            ;;
        test/creditWithdraw/credit_withdraw_test.c)
            printf '%s\n' "src/creditWithdraw/credit_withdraw.c"
            ;;
        test/config/config_test.c)
            printf '%s\n' "src/config/config.c"
            ;;
        test/files/app_files_test.c)
            printf '%s\n' "src/files/app_files.c"
            ;;
        test/offerings/offerings_test.c)
            printf '%s\n' "src/offerings/offerings.c"
            ;;
        test/sharedStorage/shared_storage_test.c)
            printf '%s\n' "src/sharedStorage/shared_storage.c"
            printf '%s\n' "src/sharedStorage/fileSystem/shared_storage_file_system.c"
            printf '%s\n' "src/sharedStorage/management/shared_storage_management.c"
            ;;
        *)
            printf 'Unknown C test source: %s\n' "$test_source_path" >&2
            return 1
            ;;
    esac
}

run_c_test() {
    local test_source_path="$1"
    local test_binary_path
    local module_source_files

    test_binary_path="$(test_binary_path_for "$test_source_path")"
    if ! module_source_files="$(source_files_for "$test_source_path")"; then
        failure_count=$((failure_count + 1))
        return
    fi

    mkdir -p "$test_build_directory"

    printf '=== RUN %s ===\n' "$test_source_path"

    if ! gcc -Wall -Wextra -Wpedantic -std=c11 -Isrc -Itest "$test_source_path" $module_source_files -o "$test_binary_path" -ldl; then
        printf 'not ok - %s (compile failed)\n' "$test_source_path"
        printf '=== FAIL %s ===\n' "$test_source_path"
        failure_count=$((failure_count + 1))
        return
    fi

    local test_output
    local test_exit_code

    test_output="$(TALKSPHERE_LOG_LEVEL=fatal "$test_binary_path" 2>&1)"
    test_exit_code="$?"
    if [ "$test_exit_code" -ne 0 ]; then
        printf 'not ok - %s\n' "$test_source_path"
        printf 'exit code: %s\n' "$test_exit_code"
        printf '%s\n' "$test_output"
        printf '=== FAIL %s ===\n' "$test_source_path"
        failure_count=$((failure_count + 1))
        return
    fi

    printf 'ok - %s\n' "$test_source_path"
    printf '=== PASS %s ===\n' "$test_source_path"
}

run_shell_test() {
    local test_script_path="$1"
    local test_output
    local test_exit_code

    printf '=== RUN %s ===\n' "$test_script_path"

    test_output="$(bash "$test_script_path" 2>&1)"
    test_exit_code="$?"
    if [ "$test_exit_code" -ne 0 ]; then
        printf 'not ok - %s\n' "$test_script_path"
        printf 'exit code: %s\n' "$test_exit_code"
        printf '%s\n' "$test_output"
        printf '=== FAIL %s ===\n' "$test_script_path"
        failure_count=$((failure_count + 1))
        return
    fi

    printf 'ok - %s\n' "$test_script_path"
    printf '=== PASS %s ===\n' "$test_script_path"
}

run_test_path() {
    local test_path="$1"

    case "$test_path" in
        *.c)
            run_c_test "$test_path"
            ;;
        *.sh)
            run_shell_test "$test_path"
            ;;
        *)
            printf 'Unknown test file type: %s\n' "$test_path" >&2
            failure_count=$((failure_count + 1))
            ;;
    esac
}

if [ "$#" -eq 0 ]; then
    for test_path in "${all_test_paths[@]}"; do
        run_test_path "$test_path"
    done
else
    for requested_test_path in "$@"; do
        normalized_test_path="$(normalize_test_path "$requested_test_path")"
        run_test_path "$normalized_test_path"
    done
fi

if [ "$failure_count" -ne 0 ]; then
    exit 1
fi
