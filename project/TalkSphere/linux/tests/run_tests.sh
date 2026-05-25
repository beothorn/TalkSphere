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
        printf 'not ok - %s\n%s\n' "$test_name" "$output"
        failure_count=$((failure_count + 1))
        return
    fi

    if [[ "$output" != *"$expected_text"* ]]; then
        printf 'not ok - %s\nexpected text: %s\n%s\n' "$test_name" "$expected_text" "$output"
        failure_count=$((failure_count + 1))
        return
    fi

    printf 'ok - %s\n' "$test_name"
}

run_expect_scenario_success() {
    local listener_port=18999
    local instance_one_port=18734
    local instance_two_port=18735

    local listener_output_file
    listener_output_file="$(mktemp)"

    local instance_one_log_file
    instance_one_log_file="$(mktemp)"

    local instance_two_log_file
    instance_two_log_file="$(mktemp)"

    timeout 8 nc -l "$listener_port" > "$listener_output_file" &
    local listener_process_id=$!

    TALKSPHERE_LOG_LEVEL=warn timeout 8 "$binary_path" "$instance_one_port" 19901 > "$instance_one_log_file" 2>&1 &
    local instance_one_process_id=$!

    TALKSPHERE_LOG_LEVEL=warn timeout 8 "$binary_path" "$instance_two_port" 19902 > "$instance_two_log_file" 2>&1 &
    local instance_two_process_id=$!

    sleep 1
    printf 'CONNECT:localhost:%d,FROM:localhost:%d' "$instance_two_port" "$listener_port" | nc -N localhost "$instance_one_port"

    wait "$listener_process_id"
    wait "$instance_one_process_id"
    wait "$instance_two_process_id"

    if ! grep -q "MESSAGE:Hello" "$listener_output_file"; then
        printf 'not ok - two-instance connect scenario (listener did not receive MESSAGE:Hello)\n'
        cat "$listener_output_file"
        failure_count=$((failure_count + 1))
        return
    fi

    if ! grep -q "^Hello$" "$instance_two_log_file"; then
        printf 'not ok - two-instance connect scenario (instance two did not print Hello)\n'
        cat "$instance_two_log_file"
        failure_count=$((failure_count + 1))
        return
    fi

    printf 'ok - two-instance connect scenario\n'
}

run_expect_scenario_success
run_expect_failure "invalid client port" "Invalid client port: bad" bad 9898
run_expect_failure "invalid server port" "Invalid server port: bad" 8999 bad
run_expect_failure "same client and server port" "Client and server ports must be different." 8999 8999

if [ "$failure_count" -ne 0 ]; then
    exit 1
fi
