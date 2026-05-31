#!/usr/bin/env bash
set -u

binary_path="./talksphere"
failure_count=0

run_expect_failure() {
    local test_name="$1"
    local expected_text="$2"
    shift
    shift
    local output
    if output="$($binary_path "$@" 2>&1)"; then
        printf 'not ok - %s\n%s\n' "$test_name" "$output"; failure_count=$((failure_count + 1)); return
    fi
    if [[ "$output" != *"$expected_text"* ]]; then
        printf 'not ok - %s\nexpected text: %s\n%s\n' "$test_name" "$expected_text" "$output"; failure_count=$((failure_count + 1)); return
    fi
    printf 'ok - %s\n' "$test_name"
}

run_expect_identifier_creation() {
    local temporary_root; temporary_root="$(mktemp -d)"
    local app_directory_path="$temporary_root/data/talksphere"
    mkdir -p "$temporary_root/data"
    TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root/data" timeout 2 "$binary_path" >/dev/null 2>&1 || true
    [[ -d "$app_directory_path" ]] || { printf 'not ok - creates app directory and identifier (directory not created)\n'; failure_count=$((failure_count+1)); return; }
    [[ -d "$app_directory_path/ledger" ]] || { printf 'not ok - creates app directory and identifier (ledger not created)\n'; failure_count=$((failure_count+1)); return; }
    [[ -f "$app_directory_path/id" ]] || { printf 'not ok - creates app directory and identifier (id not created)\n'; failure_count=$((failure_count+1)); return; }
    local identifier_text; identifier_text="$(cat "$app_directory_path/id")"
    if [[ ! "$identifier_text" =~ ^[A-Za-z0-9_-]{128}$ ]]; then printf 'not ok - creates app directory and identifier (invalid id format)\n'; failure_count=$((failure_count+1)); return; fi
    printf 'ok - creates app directory and identifier\n'
}

run_two_instance_credit_scenario() {
    local test_name="two instances pay then message spends credits"
    local temporary_root; temporary_root="$(mktemp -d)"
    local instance_one_data="$temporary_root/instance1"
    local instance_two_data="$temporary_root/instance2"
    mkdir -p "$instance_one_data" "$instance_two_data"

    TALKSPHERE_LOG_LEVEL=warn timeout 8 "$binary_path" 9101 9102 "$instance_one_data" >"$temporary_root/instance1.log" 2>&1 &
    local instance_one_pid=$!
    TALKSPHERE_LOG_LEVEL=warn timeout 8 "$binary_path" 9201 9202 "$instance_two_data" >"$temporary_root/instance2.log" 2>&1 &
    local instance_two_pid=$!
    sleep 1

    local receiver_identifier
    receiver_identifier="$(cat "$instance_two_data/id")"

    printf 'PAY:%s' "$receiver_identifier" | nc -N localhost 9201
    sleep 0.2
    printf 'MESSAGE:paid hello' | nc -N localhost 9201
    sleep 0.3

    local remaining_credits
    remaining_credits="$(cat "$instance_two_data/ledger/$receiver_identifier" 2>/dev/null || echo "missing")"

    kill "$instance_one_pid" "$instance_two_pid" >/dev/null 2>&1 || true
    wait "$instance_one_pid" "$instance_two_pid" >/dev/null 2>&1 || true

    if [[ "$remaining_credits" != "0" ]]; then
        printf 'not ok - %s (expected remaining credits 0, got %s)\n' "$test_name" "$remaining_credits"
        failure_count=$((failure_count+1))
        return
    fi

    if ! grep -q "paid hello" "$temporary_root/instance2.log"; then
        printf 'not ok - %s (message not printed)\n' "$test_name"
        failure_count=$((failure_count+1))
        return
    fi

    printf 'ok - %s\n' "$test_name"
}

run_expect_ledger_summary() {
    local test_name="prints ledger owned and owed credits"
    local temporary_root; temporary_root="$(mktemp -d)"
    local app_directory_path="$temporary_root/app"

    local initial_output
    if ! initial_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --ledger-summary "$app_directory_path" 2>&1)"; then
        printf 'not ok - %s (initial summary command failed)\n%s\n' "$test_name" "$initial_output"
        failure_count=$((failure_count+1))
        return
    fi

    local local_identifier
    local_identifier="$(cat "$app_directory_path/id")"
    printf '3' > "$app_directory_path/ledger/$local_identifier"
    printf '4' > "$app_directory_path/ledger/other-peer"

    local summary_output
    if ! summary_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --ledger-summary "$app_directory_path" 2>&1)"; then
        printf 'not ok - %s (summary command failed)\n%s\n' "$test_name" "$summary_output"
        failure_count=$((failure_count+1))
        return
    fi

    if [[ "$summary_output" != *"Owned credits: 3"* || "$summary_output" != *"Owed credits: 4"* ]]; then
        printf 'not ok - %s (unexpected summary)\n%s\n' "$test_name" "$summary_output"
        failure_count=$((failure_count+1))
        return
    fi

    printf 'ok - %s\n' "$test_name"
}


run_expect_encryption_placeholders() {
    local test_name="encryption placeholders return empty outputs and reject missing output sizes"
    local temporary_root; temporary_root="$(mktemp -d)"
    local test_source_path="$temporary_root/encryption_placeholder_test.c"
    local test_binary_path="$temporary_root/encryption_placeholder_test"

    cat > "$test_source_path" <<'TESTSOURCE'
#include "encryption/encryption.h"
#include "common/result.h"

#include <stddef.h>

int main(void) {
    unsigned char public_key_bytes[1];
    unsigned char private_key_bytes[1];
    unsigned char encrypted_message_bytes[1];
    unsigned char signature_bytes[1];
    const unsigned char message_bytes[] = "hello";
    size_t public_key_size = 99;
    size_t private_key_size = 99;
    size_t encrypted_message_size = 99;
    size_t signature_size = 99;

    if (create_encryption_keys(
            public_key_bytes,
            sizeof(public_key_bytes),
            &public_key_size,
            private_key_bytes,
            sizeof(private_key_bytes),
            &private_key_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return 1;
    }

    if (public_key_size != 0 || private_key_size != 0) {
        return 2;
    }

    if (encrypt_message(
            public_key_bytes,
            public_key_size,
            message_bytes,
            sizeof(message_bytes),
            encrypted_message_bytes,
            sizeof(encrypted_message_bytes),
            &encrypted_message_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return 3;
    }

    if (encrypted_message_size != 0) {
        return 4;
    }

    if (sign_message(
            private_key_bytes,
            private_key_size,
            message_bytes,
            sizeof(message_bytes),
            signature_bytes,
            sizeof(signature_bytes),
            &signature_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return 5;
    }

    if (signature_size != 0) {
        return 6;
    }

    if (create_encryption_keys(
            public_key_bytes,
            sizeof(public_key_bytes),
            NULL,
            private_key_bytes,
            sizeof(private_key_bytes),
            &private_key_size
        ) != TALKSPHERE_FAILURE
    ) {
        return 7;
    }

    if (encrypt_message(
            public_key_bytes,
            public_key_size,
            message_bytes,
            sizeof(message_bytes),
            encrypted_message_bytes,
            sizeof(encrypted_message_bytes),
            NULL
        ) != TALKSPHERE_FAILURE
    ) {
        return 8;
    }

    if (sign_message(
            private_key_bytes,
            private_key_size,
            message_bytes,
            sizeof(message_bytes),
            signature_bytes,
            sizeof(signature_bytes),
            NULL
        ) != TALKSPHERE_FAILURE
    ) {
        return 9;
    }

    return 0;
}
TESTSOURCE

    if ! gcc -Wall -Wextra -Wpedantic -std=c11 -Isrc "$test_source_path" src/encryption/encryption.c -o "$test_binary_path"; then
        printf 'not ok - %s (compile failed)\n' "$test_name"
        failure_count=$((failure_count+1))
        return
    fi

    if ! TALKSPHERE_LOG_LEVEL=fatal "$test_binary_path"; then
        printf 'not ok - %s (placeholder behavior failed)\n' "$test_name"
        failure_count=$((failure_count+1))
        return
    fi

    printf 'ok - %s\n' "$test_name"
}

run_expect_identifier_creation
run_two_instance_credit_scenario
run_expect_ledger_summary
run_expect_encryption_placeholders
run_expect_failure "invalid client port" "Invalid client port: bad" bad 9898
run_expect_failure "invalid server port" "Invalid server port: bad" 8999 bad
run_expect_failure "same client and server port" "Client and server ports must be different." 8999 8999
run_expect_failure "ledger summary with too many arguments" "Usage:" --ledger-summary /tmp extra

if [ "$failure_count" -ne 0 ]; then exit 1; fi
