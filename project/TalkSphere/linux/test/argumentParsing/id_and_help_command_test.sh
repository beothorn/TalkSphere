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

if [[ "$help_output" != *"[-d|--directory-home <home_folder>] <command> [arguments]"*
    || "$help_output" != *"  config"$'\n'"      Manage configurations."*
    || "$help_output" != *"  talk"$'\n'"      Send commands to another TalkSphere."*
    || "$help_output" == *"config get home"*
    || "$help_output" == *"talk -p <client_port> offerings"*
]]; then
    printf 'not ok - %s (main help did not use command-family summaries)\n%s\n' "$test_name" "$help_output"
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

if ! config_help_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" config h 2>&1)"; then
    printf 'not ok - %s (config help command failed)\n%s\n' "$test_name" "$config_help_output"
    exit 1
fi

if [[ "$config_help_output" != *"config get availability"*
    || "$config_help_output" != *"config set availability"*
    || "$config_help_output" != *"config add reachableAt"*
    || "$config_help_output" != *"config remove reachableAt"*
]]; then
    printf 'not ok - %s (config help did not list expected commands)\n%s\n' \
        "$test_name" \
        "$config_help_output"
    exit 1
fi

if ! talk_help_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" talk h 2>&1)"; then
    printf 'not ok - %s (talk help command failed)\n%s\n' "$test_name" "$talk_help_output"
    exit 1
fi

if [[ "$talk_help_output" != *"talk -p <client_port> offerings"*
    || "$talk_help_output" != *"talk -p <client_port> message"*
]]; then
    printf 'not ok - %s (talk help did not list expected commands)\n%s\n' \
        "$test_name" \
        "$talk_help_output"
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

if ! directory_home_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" -d "$app_directory_path" config get home 2>&1)"; then
    printf 'not ok - %s (directory home command failed)\n%s\n' "$test_name" "$directory_home_output"
    exit 1
fi

if [[ "$directory_home_output" != "$app_directory_path" ]]; then
    printf 'not ok - %s (directory home output was unexpected)\nprinted: %s\nexpected: %s\n' \
        "$test_name" \
        "$directory_home_output" \
        "$app_directory_path"
    exit 1
fi

if ! dry_run_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" --dry-run -d "$app_directory_path" run 9001 9002 2>&1)"; then
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

if ! config_set_output="$(TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root" "$binary_path" config set availability alwaysOn 2>&1)"; then
    printf 'not ok - %s (config set command failed)\n%s\n' "$test_name" "$config_set_output"
    exit 1
fi

default_config_path="$temporary_root/talksphere/config"
if [[ "$config_set_output" != "Config availability set to alwaysOn"
    || "$(cat "$default_config_path")" != "availability=alwaysOn"
]]; then
    printf 'not ok - %s (config set output or file was unexpected)\n%s\n' \
        "$test_name" \
        "$config_set_output"
    exit 1
fi

if ! config_get_output="$(TALKSPHERE_LOG_LEVEL=warn XDG_DATA_HOME="$temporary_root" "$binary_path" config get availability 2>&1)"; then
    printf 'not ok - %s (config get availability command failed)\n%s\n' "$test_name" "$config_get_output"
    exit 1
fi

if [[ "$config_get_output" != "alwaysOn" ]]; then
    printf 'not ok - %s (config get availability output was unexpected)\n%s\n' \
        "$test_name" \
        "$config_get_output"
    exit 1
fi

custom_config_home_path="$temporary_root/config-home"
if ! config_add_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" -d "$custom_config_home_path" config add reachableAt "www.example.com:9999" 2>&1)"; then
    printf 'not ok - %s (config add command failed)\n%s\n' "$test_name" "$config_add_output"
    exit 1
fi

if ! config_remove_output="$(TALKSPHERE_LOG_LEVEL=warn "$binary_path" -d "$custom_config_home_path" config remove reachableAt "www.example.com:9999" 2>&1)"; then
    printf 'not ok - %s (config remove command failed)\n%s\n' "$test_name" "$config_remove_output"
    exit 1
fi

custom_config_path="$custom_config_home_path/config"
if [[ "$config_add_output" != "Config reachableAt added www.example.com:9999"
    || "$config_remove_output" != "Config reachableAt removed www.example.com:9999"
    || -s "$custom_config_path"
]]; then
    printf 'not ok - %s (config add/remove output or file was unexpected)\nadd: %s\nremove: %s\n' \
        "$test_name" \
        "$config_add_output" \
        "$config_remove_output"
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
