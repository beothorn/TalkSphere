#!/usr/bin/env bash
set -u

binary_path="./talksphere"
failure_count=0

run_expect_success() {
    local test_name="$1"
    local expected_text="$2"
    shift
    shift

    local output
    if ! output="$("$binary_path" "$@" 2>&1)"; then
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

run_expect_failure() {
    local test_name="$1"
    local expected_text="$2"
    shift
    shift

    local output
    if output="$("$binary_path" "$@" 2>&1)"; then
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

run_expect_log_hidden_by_default() {
    local output
    if ! output="$("$binary_path" 9101 9102 2>&1)"; then
        printf 'not ok - default log level keeps trace hidden\n%s\n' "$output"
        failure_count=$((failure_count + 1))
        return
    fi

    if [[ "$output" == *"[trace]"* ]]; then
        printf 'not ok - default log level keeps trace hidden\n%s\n' "$output"
        failure_count=$((failure_count + 1))
        return
    fi

    printf 'ok - default log level keeps trace hidden\n'
}

run_expect_trace_log_visible() {
    local output
    if ! output="$(TALKSPHERE_LOG_LEVEL=trace "$binary_path" 9103 9104 2>&1)"; then
        printf 'not ok - trace log level shows trace logs\n%s\n' "$output"
        failure_count=$((failure_count + 1))
        return
    fi

    if [[ "$output" != *"[trace]"* ]]; then
        printf 'not ok - trace log level shows trace logs\n%s\n' "$output"
        failure_count=$((failure_count + 1))
        return
    fi

    printf 'ok - trace log level shows trace logs\n'
}

run_expect_success "default ports" "Received from 127.0.0.1:8512 -> Hello from TalkSphere"
run_expect_success "custom ports" "Received from 127.0.0.1:8999 -> Hello from TalkSphere" 8999 9898
run_expect_failure "invalid client port" "Invalid client port: bad" bad 9898
run_expect_failure "invalid server port" "Invalid server port: bad" 8999 bad
run_expect_failure "same client and server port" "Client and server ports must be different." 8999 8999
run_expect_log_hidden_by_default
run_expect_trace_log_visible

if [ "$failure_count" -ne 0 ]; then
    exit 1
fi
