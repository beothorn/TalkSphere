#!/usr/bin/env bash
set -u

binary_path="./build/talksphere"
test_name="two instances pay then message spends credits"
temporary_root="$(mktemp -d)"
instance_one_data="$temporary_root/instance1"
instance_two_data="$temporary_root/instance2"

mkdir -p "$instance_one_data" "$instance_two_data"

TALKSPHERE_LOG_LEVEL=warn timeout 8 "$binary_path" 9101 9102 "$instance_one_data" >"$temporary_root/instance1.log" 2>&1 &
instance_one_process_id=$!
TALKSPHERE_LOG_LEVEL=warn timeout 8 "$binary_path" 9201 9202 "$instance_two_data" >"$temporary_root/instance2.log" 2>&1 &
instance_two_process_id=$!

sleep 1

receiver_identifier="$(cat "$instance_two_data/id")"

printf 'PAY:%s' "$receiver_identifier" | nc -N localhost 9201
sleep 0.2
printf 'MESSAGE:paid hello' | nc -N localhost 9201
sleep 0.3

remaining_credits="$(cat "$instance_two_data/ledger/$receiver_identifier" 2>/dev/null || echo "missing")"

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
