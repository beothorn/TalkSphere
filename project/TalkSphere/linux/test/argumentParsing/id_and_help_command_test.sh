#!/usr/bin/env bash
set -u

binary_path="./build/talksphere"
test_name="prints id home and help"
temporary_root="$(mktemp -d)"
app_directory_path="$temporary_root/app"

if ! help_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --help 2>&1)"; then
    printf 'not ok - %s (help command failed)\n%s\n' "$test_name" "$help_output"
    exit 1
fi

if [[ "$help_output" != *"--id [storage_directory]"*
    || "$help_output" != *"--home [storage_directory]"*
    || "$help_output" != *"--help"*
    || "$help_output" != *"--ledger-summary [storage_directory]"*
]]; then
    printf 'not ok - %s (help did not list expected arguments)\n%s\n' "$test_name" "$help_output"
    exit 1
fi

if [[ -e "$app_directory_path" ]]; then
    printf 'not ok - %s (help unexpectedly created storage)\n' "$test_name"
    exit 1
fi

if ! identifier_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --id "$app_directory_path" 2>&1)"; then
    printf 'not ok - %s (id command failed)\n%s\n' "$test_name" "$identifier_output"
    exit 1
fi

stored_identifier="$(cat "$app_directory_path/id")"
if [[ "$identifier_output" != "$stored_identifier" ]]; then
    printf 'not ok - %s (printed id does not match stored id)\nprinted: %s\nstored: %s\n' \
        "$test_name" \
        "$identifier_output" \
        "$stored_identifier"
    exit 1
fi

if [[ ! "$identifier_output" =~ ^[A-Za-z0-9_-]{128}$ ]]; then
    printf 'not ok - %s (printed id has unexpected shape)\n%s\n' "$test_name" "$identifier_output"
    exit 1
fi

if ! home_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --home "$app_directory_path" 2>&1)"; then
    printf 'not ok - %s (home command failed)\n%s\n' "$test_name" "$home_output"
    exit 1
fi

if [[ "$home_output" != "$app_directory_path" ]]; then
    printf 'not ok - %s (printed home does not match storage directory)\nprinted: %s\nexpected: %s\n' \
        "$test_name" \
        "$home_output" \
        "$app_directory_path"
    exit 1
fi

if [[ ! -f "$home_output/id" || ! -f "$home_output/offerings" || ! -d "$home_output/ledger" ]]; then
    printf 'not ok - %s (home does not contain expected app files)\n%s\n' "$test_name" "$home_output"
    exit 1
fi
