#!/usr/bin/env bash
set -u

binary_path="./talksphere"
failure_count=0

run_expect_identifier_creation() {
    local test_name="creates app directory and identifier"
    local temporary_root
    temporary_root="$(mktemp -d)"

    local app_directory_path="$temporary_root/data/talksphere"
    mkdir -p "$temporary_root/data"

    TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root/data" timeout 2 "$binary_path" >/dev/null 2>&1 || true

    if [[ ! -d "$app_directory_path" ]]; then
        printf 'not ok - %s (directory was not created)\n' "$test_name"
        failure_count=$((failure_count + 1))
        return
    fi

    if [[ ! -f "$app_directory_path/id" ]]; then
        printf 'not ok - %s (id file was not created)\n' "$test_name"
        failure_count=$((failure_count + 1))
        return
    fi

    local identifier_text
    identifier_text="$(cat "$app_directory_path/id")"

    if [[ ! "$identifier_text" =~ ^[A-Za-z0-9_-]{22}$ ]]; then
        printf 'not ok - %s (id format is invalid: %s)\n' "$test_name" "$identifier_text"
        failure_count=$((failure_count + 1))
        return
    fi

    printf 'ok - %s\n' "$test_name"
}

run_expect_identifier_kept() {
    local test_name="keeps existing identifier"
    local temporary_root
    temporary_root="$(mktemp -d)"

    local app_directory_path="$temporary_root/data/talksphere"
    mkdir -p "$app_directory_path"

    local existing_identifier="AAAAAAAAAAAAAAAAAAAAAA"
    printf '%s' "$existing_identifier" > "$app_directory_path/id"

    TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root/data" timeout 2 "$binary_path" >/dev/null 2>&1 || true

    local resulting_identifier
    resulting_identifier="$(cat "$app_directory_path/id")"

    if [[ "$resulting_identifier" != "$existing_identifier" ]]; then
        printf 'not ok - %s (existing id was modified)\n' "$test_name"
        failure_count=$((failure_count + 1))
        return
    fi

    printf 'ok - %s\n' "$test_name"
}

run_expect_identifier_creation
run_expect_identifier_kept

if [ "$failure_count" -ne 0 ]; then
    exit 1
fi
