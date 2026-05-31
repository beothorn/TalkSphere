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

run_expect_identifier_creation
run_two_instance_credit_scenario
run_expect_failure "invalid client port" "Invalid client port: bad" bad 9898
run_expect_failure "invalid server port" "Invalid server port: bad" 8999 bad
run_expect_failure "same client and server port" "Client and server ports must be different." 8999 8999

if [ "$failure_count" -ne 0 ]; then exit 1; fi
