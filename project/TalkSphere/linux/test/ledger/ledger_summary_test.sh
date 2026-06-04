#!/usr/bin/env bash
set -u

binary_path="./build/talksphere"
test_name="prints ledger owned and owed credits"
temporary_root="$(mktemp -d)"
app_directory_path="$temporary_root/app"

if ! initial_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --ledger-summary "$app_directory_path" 2>&1)"; then
    printf 'not ok - %s (initial summary command failed)\n%s\n' "$test_name" "$initial_output"
    exit 1
fi

local_identifier="$(cat "$app_directory_path/id")"
printf '3' > "$app_directory_path/ledger/$local_identifier"
printf '4' > "$app_directory_path/ledger/other-peer"

if ! summary_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --ledger-summary "$app_directory_path" 2>&1)"; then
    printf 'not ok - %s (summary command failed)\n%s\n' "$test_name" "$summary_output"
    exit 1
fi

if [[ "$summary_output" != *"Owned credits: 3"* || "$summary_output" != *"Owed credits: 4"* ]]; then
    printf 'not ok - %s (unexpected summary)\n%s\n' "$test_name" "$summary_output"
    exit 1
fi
