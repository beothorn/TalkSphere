#!/usr/bin/env bash
set -u

binary_path="./build/talksphere"
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
    [[ -f "$app_directory_path/offerings" ]] || { printf 'not ok - creates app directory and identifier (offerings not created)\n'; failure_count=$((failure_count+1)); return; }
    local identifier_text; identifier_text="$(cat "$app_directory_path/id")"
    if [[ ! "$identifier_text" =~ ^[A-Za-z0-9_-]{128}$ ]]; then printf 'not ok - creates app directory and identifier (invalid id format)\n'; failure_count=$((failure_count+1)); return; fi
    local offerings_text; offerings_text="$(cat "$app_directory_path/offerings")"
    if [[ "$offerings_text" != *'"availability": "alwaysOn"'* || "$offerings_text" != *'"type":"askForMessages"'* ]]; then printf 'not ok - creates app directory and identifier (invalid offerings)\n'; failure_count=$((failure_count+1)); return; fi
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

run_expect_shared_storage_placeholders() {
    local test_name="shared storage placeholders validate inputs"
    local temporary_root; temporary_root="$(mktemp -d)"
    local test_source_path="$temporary_root/shared_storage_placeholder_test.c"
    local test_binary_path="$temporary_root/shared_storage_placeholder_test"

    cat > "$test_source_path" <<'TESTSOURCE'
#include "sharedStorage/shared_storage.h"
#include "common/result.h"

#include <stddef.h>

int main(void) {
    const char *app_storage_directory_path = "/tmp/talksphere-shared-storage-test";

    if (shared_storage_share_available_storage(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return 1;
    }

    if (shared_storage_recover_sold_storage(app_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return 2;
    }

    if (shared_storage_clear_aged_storage(app_storage_directory_path, 60) != TALKSPHERE_SUCCESS) {
        return 3;
    }

    if (shared_storage_share_available_storage(NULL) != TALKSPHERE_FAILURE) {
        return 4;
    }

    if (shared_storage_recover_sold_storage(NULL) != TALKSPHERE_FAILURE) {
        return 5;
    }

    if (shared_storage_clear_aged_storage(app_storage_directory_path, -1) != TALKSPHERE_FAILURE) {
        return 6;
    }

    return 0;
}
TESTSOURCE

    if ! gcc -Wall -Wextra -Wpedantic -std=c11 -Isrc "$test_source_path" src/sharedStorage/shared_storage.c -o "$test_binary_path"; then
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

run_expect_offerings_reader() {
    local test_name="offerings reader returns local file text and validates errors"
    local temporary_root; temporary_root="$(mktemp -d)"
    local test_source_path="$temporary_root/offerings_reader_test.c"
    local test_binary_path="$temporary_root/offerings_reader_test"

    cat > "$test_source_path" <<'TESTSOURCE'
#include "offerings/offerings.h"
#include "common/result.h"

#include <stdio.h>
#include <string.h>

int main(
    int argument_count,
    char *argument_values[]
) {
    if (argument_count != 2) {
        return 1;
    }

    const char *app_storage_directory_path = argument_values[1];
    char offerings_text[256];

    if (read_local_offerings(
            app_storage_directory_path,
            offerings_text,
            strlen("[{\"availability\":\"alwaysOn\"}]") + 1
        ) != TALKSPHERE_SUCCESS
    ) {
        return 8;
    }

    if (strcmp(
            offerings_text,
            "[{\"availability\":\"alwaysOn\"}]"
        ) != 0
    ) {
        return 9;
    }

    char missing_file_directory_path[256];
    if (snprintf(
            missing_file_directory_path,
            sizeof(missing_file_directory_path),
            "%s-missing",
            app_storage_directory_path
        ) < 0
    ) {
        return 10;
    }

    if (read_local_offerings(
            missing_file_directory_path,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return 11;
    }

    if (read_local_offerings(
            app_storage_directory_path,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return 2;
    }

    if (strcmp(
            offerings_text,
            "[{\"availability\":\"alwaysOn\"}]"
        ) != 0
    ) {
        return 3;
    }

    char small_offerings_text[4];
    if (read_local_offerings(
            app_storage_directory_path,
            small_offerings_text,
            sizeof(small_offerings_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return 4;
    }

    if (read_local_offerings(
            NULL,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return 5;
    }

    if (read_local_offerings(
            app_storage_directory_path,
            NULL,
            sizeof(offerings_text)
        ) != TALKSPHERE_FAILURE
    ) {
        return 6;
    }

    if (read_local_offerings(
            app_storage_directory_path,
            offerings_text,
            0
        ) != TALKSPHERE_FAILURE
    ) {
        return 7;
    }

    return 0;
}
TESTSOURCE

    mkdir -p "$temporary_root/app"
    printf '[{"availability":"alwaysOn"}]' > "$temporary_root/app/offerings"

    if ! gcc -Wall -Wextra -Wpedantic -std=c11 -Isrc "$test_source_path" src/offerings/offerings.c -o "$test_binary_path"; then
        printf 'not ok - %s (compile failed)\n' "$test_name"
        failure_count=$((failure_count+1))
        return
    fi

    if ! TALKSPHERE_LOG_LEVEL=fatal "$test_binary_path" "$temporary_root/app"; then
        printf 'not ok - %s (reader behavior failed)\n' "$test_name"
        failure_count=$((failure_count+1))
        return
    fi

    printf 'ok - %s\n' "$test_name"
}

run_expect_identifier_creation
run_two_instance_credit_scenario
run_expect_ledger_summary
run_expect_encryption_placeholders
run_expect_shared_storage_placeholders
run_expect_offerings_reader
run_expect_failure "invalid client port" "Invalid client port: bad" bad 9898
run_expect_failure "invalid server port" "Invalid server port: bad" 8999 bad
run_expect_failure "same client and server port" "Client and server ports must be different." 8999 8999
run_expect_failure "ledger summary with too many arguments" "Usage:" --ledger-summary /tmp extra

if [ "$failure_count" -ne 0 ]; then exit 1; fi
