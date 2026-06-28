#!/usr/bin/env bash
set -u

binary_path="./build/talksphere"
test_name="socket commands"
temporary_root="$(mktemp -d)"
instance_one_data="$temporary_root/instance1"
instance_two_data="$temporary_root/instance2"

mkdir -p "$instance_one_data" "$instance_two_data"

TALKSPHERE_LOG_LEVEL=warn timeout 8 "$binary_path" --home "$instance_one_data" run 9101 9201 >"$temporary_root/instance1.log" 2>&1 &
instance_one_process_id=$!
TALKSPHERE_LOG_LEVEL=warn timeout 8 "$binary_path" --home "$instance_two_data" run 9201 9202 >"$temporary_root/instance2.log" 2>&1 &
instance_two_process_id=$!

sleep 1

receiver_identifier="$(cat "$instance_two_data/id")"

printf 'PAY:%s' "$receiver_identifier" | nc -N localhost 9201
sleep 0.2
TALKSPHERE_LOG_LEVEL=warn "$binary_path" talk -p 9101 message "paid hello"
sleep 0.3

remaining_credits="$(cat "$instance_two_data/ledger/$receiver_identifier" 2>/dev/null || echo "missing")"
custom_offerings='{"availability":"changedAtHome","reachableAt":["localhost:9201"],"buy":[],"sell":[]}'
printf '%s' "$custom_offerings" >"$instance_two_data/offerings"
offerings_response="$(printf 'LIST_OFFERINGS' | nc -N localhost 9201)"
talk_offerings_response="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" talk -p 9101 offerings)"

kill "$instance_one_process_id" "$instance_two_process_id" >/dev/null 2>&1 || true
wait "$instance_one_process_id" "$instance_two_process_id" >/dev/null 2>&1 || true

if [[ "$remaining_credits" != "0" ]]; then
    printf 'not ok - %s (expected remaining credits 0, got %s)\n' "$test_name" "$remaining_credits"
    exit 1
fi

if ! grep -q "paid hello" "$temporary_root/instance2.log"; then
    printf 'not ok - %s (message not printed)\n' "$test_name"
    exit 1
fi

if [[ "$offerings_response" != "$custom_offerings" ]]; then
    printf 'not ok - %s (LIST_OFFERINGS did not return current home offerings)\nexpected: %s\nreceived: %s\n' \
        "$test_name" \
        "$custom_offerings" \
        "$offerings_response"
    exit 1
fi

if [[ "$talk_offerings_response" != "$custom_offerings" ]]; then
    printf 'not ok - %s (talk command did not return connected peer offerings)\nexpected: %s\nreceived: %s\n' \
        "$test_name" \
        "$custom_offerings" \
        "$talk_offerings_response"
    exit 1
fi
