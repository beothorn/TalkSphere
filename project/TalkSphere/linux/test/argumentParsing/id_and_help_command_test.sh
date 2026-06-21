#!/usr/bin/env bash
set -u

binary_path="./build/talksphere"
test_name="prints command help home and placeholders"
temporary_root="$(mktemp -d)"
app_directory_path="$temporary_root/app"

if ! help_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --help 2>&1)"; then
    printf 'not ok - %s (help command failed)\n%s\n' "$test_name" "$help_output"
    exit 1
fi

if [[ "$help_output" != *"run <listen_port> <peer_port> [home_folder]"*
    || "$help_output" != *"config get home"*
    || "$help_output" != *"encryption [--help|help|h]"*
    || "$help_output" != *"share [--help|help|h]"*
]]; then
    printf 'not ok - %s (help did not list expected commands)\n%s\n' "$test_name" "$help_output"
    exit 1
fi

if [[ -e "$app_directory_path" ]]; then
    printf 'not ok - %s (help unexpectedly created storage)\n' "$test_name"
    exit 1
fi

if ! encryption_help_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" encryption h 2>&1)"; then
    printf 'not ok - %s (encryption help command failed)\n%s\n' "$test_name" "$encryption_help_output"
    exit 1
fi

if [[ "$encryption_help_output" != *"encryption create"*
    || "$encryption_help_output" != *"encryption recreate"*
    || "$encryption_help_output" != *"encryption encrypt_message"*
    || "$encryption_help_output" != *"encryption sign_message"*
]]; then
    printf 'not ok - %s (encryption help did not list expected commands)\n%s\n' \
        "$test_name" \
        "$encryption_help_output"
    exit 1
fi

if ! home_output="$(TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root" "$binary_path" config get home 2>&1)"; then
    printf 'not ok - %s (config home command failed)\n%s\n' "$test_name" "$home_output"
    exit 1
fi

expected_home_output="$temporary_root/talksphere"
if [[ "$home_output" != "$expected_home_output" ]]; then
    printf 'not ok - %s (printed home does not match storage directory)\nprinted: %s\nexpected: %s\n' \
        "$test_name" \
        "$home_output" \
        "$expected_home_output"
    exit 1
fi

if [[ -e "$home_output" ]]; then
    printf 'not ok - %s (config home unexpectedly created storage)\n%s\n' "$test_name" "$home_output"
    exit 1
fi

if ! dry_run_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --dry-run run 9001 9002 "$app_directory_path" 2>&1)"; then
    printf 'not ok - %s (dry run command failed)\n%s\n' "$test_name" "$dry_run_output"
    exit 1
fi

if [[ "$dry_run_output" != *"Would run TalkSphere with listen port 9001, peer port 9002"* ]]; then
    printf 'not ok - %s (dry run output was unexpected)\n%s\n' "$test_name" "$dry_run_output"
    exit 1
fi

if [[ -e "$app_directory_path" ]]; then
    printf 'not ok - %s (dry run unexpectedly created storage)\n' "$test_name"
    exit 1
fi

if ! encryption_dry_run_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --dry-run encryption sign_message hello 2>&1)"; then
    printf 'not ok - %s (encryption dry run command failed)\n%s\n' "$test_name" "$encryption_dry_run_output"
    exit 1
fi

if [[ "$encryption_dry_run_output" != "Would sign message: hello" ]]; then
    printf 'not ok - %s (encryption dry run output was unexpected)\n%s\n' \
        "$test_name" \
        "$encryption_dry_run_output"
    exit 1
fi

if ! offerings_dry_run_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --dry-run offerings add coffee 2>&1)"; then
    printf 'not ok - %s (offerings dry run command failed)\n%s\n' "$test_name" "$offerings_dry_run_output"
    exit 1
fi

if [[ "$offerings_dry_run_output" != "Would add offering: coffee" ]]; then
    printf 'not ok - %s (offerings dry run output was unexpected)\n%s\n' \
        "$test_name" \
        "$offerings_dry_run_output"
    exit 1
fi

if ! share_dry_run_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --dry-run share local ls 2>&1)"; then
    printf 'not ok - %s (share dry run command failed)\n%s\n' "$test_name" "$share_dry_run_output"
    exit 1
fi

if [[ "$share_dry_run_output" != "Would list local shared storage metadata" ]]; then
    printf 'not ok - %s (share dry run output was unexpected)\n%s\n' \
        "$test_name" \
        "$share_dry_run_output"
    exit 1
fi

if ! credit_dry_run_output="$(TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root" "$binary_path" --dry-run credit add 7 gift-code 2>&1)"; then
    printf 'not ok - %s (credit dry run command failed)\n%s\n' "$test_name" "$credit_dry_run_output"
    exit 1
fi

if [[ "$credit_dry_run_output" != "Would add 7 credit withdraw code gift-code in $temporary_root/talksphere" ]]; then
    printf 'not ok - %s (credit dry run output was unexpected)\n%s\n' \
        "$test_name" \
        "$credit_dry_run_output"
    exit 1
fi

if [[ -e "$app_directory_path" ]]; then
    printf 'not ok - %s (module dry runs unexpectedly created storage)\n' "$test_name"
    exit 1
fi

if ! create_output="$(TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root" "$binary_path" encryption create 2>&1)"; then
    printf 'not ok - %s (encryption create command failed)\n%s\n' "$test_name" "$create_output"
    exit 1
fi

default_home_path="$temporary_root/talksphere"
if [[ "$create_output" != "Encryption keys stored in $default_home_path" ]]; then
    printf 'not ok - %s (encryption create output was unexpected)\n%s\n' "$test_name" "$create_output"
    exit 1
fi

if [[ ! -f "$default_home_path/encryption_public.key"
    || ! -f "$default_home_path/encryption_private.key"
]]; then
    printf 'not ok - %s (encryption key files were not created)\n%s\n' "$test_name" "$default_home_path"
    exit 1
fi

if create_again_output="$(TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root" "$binary_path" encryption create 2>&1)"; then
    printf 'not ok - %s (encryption create should fail when keys exist)\n%s\n' "$test_name" "$create_again_output"
    exit 1
fi

if ! placeholder_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" network ping 127.0.0.1:8513 2>&1)"; then
    printf 'not ok - %s (network ping placeholder failed)\n%s\n' "$test_name" "$placeholder_output"
    exit 1
fi

if [[ "$placeholder_output" != "network ping is not implemented yet" ]]; then
    printf 'not ok - %s (placeholder output was unexpected)\n%s\n' "$test_name" "$placeholder_output"
    exit 1
fi

if ! offerings_placeholder_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" offerings add coffee 2>&1)"; then
    printf 'not ok - %s (offerings placeholder failed)\n%s\n' "$test_name" "$offerings_placeholder_output"
    exit 1
fi

if [[ "$offerings_placeholder_output" != "offering add is not implemented yet" ]]; then
    printf 'not ok - %s (offerings placeholder output was unexpected)\n%s\n' \
        "$test_name" \
        "$offerings_placeholder_output"
    exit 1
fi
