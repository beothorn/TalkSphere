#include "application.h"

#include "argumentParsing/program_arguments.h"
#include "common/result.h"
#include "config/config_application.h"
#include "creditWithdraw/credit_withdraw_application.h"
#include "encryption/encryption_application.h"
#include "files/app_files.h"
#include "files/files_application.h"
#include "ledger/ledger_application.h"
#include "logging.h"
#include "network/network_application.h"
#include "offerings/offerings_application.h"
#include "sharedStorage/shared_storage_application.h"

#include <limits.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

static int command_needs_app_files(
    enum program_mode program_mode
) {
    LOG_TRACE("command_needs_app_files(): now we decide whether the command needs home files before it runs");

    return program_mode == PROGRAM_MODE_RUN_SERVER
        || program_mode == PROGRAM_MODE_START_HOME
        || program_mode == PROGRAM_MODE_GET_CONFIG_VALUE
        || program_mode == PROGRAM_MODE_SET_CONFIG_VALUE
        || program_mode == PROGRAM_MODE_ADD_CONFIG_VALUE
        || program_mode == PROGRAM_MODE_REMOVE_CONFIG_VALUE
        || program_mode == PROGRAM_MODE_PRINT_LEDGER_SUMMARY
        || program_mode == PROGRAM_MODE_CREATE_ENCRYPTION_KEYS
        || program_mode == PROGRAM_MODE_RECREATE_ENCRYPTION_KEYS
        || program_mode == PROGRAM_MODE_ENCRYPT_MESSAGE
        || program_mode == PROGRAM_MODE_SIGN_MESSAGE
        || program_mode == PROGRAM_MODE_PRINT_LOCAL_OFFERINGS
        || program_mode == PROGRAM_MODE_ADD_CREDIT_WITHDRAW_CODE
        || program_mode == PROGRAM_MODE_REMOVE_CREDIT_WITHDRAW_CODE
        || program_mode == PROGRAM_MODE_LIST_CREDIT_WITHDRAW_CODES;
}

static int program_mode_is_help(
    enum program_mode program_mode
) {
    LOG_TRACE("program_mode_is_help(): now we decide whether parsing already printed a help document");

    return program_mode == PROGRAM_MODE_PRINT_MAIN_HELP
        || program_mode == PROGRAM_MODE_PRINT_RUN_HELP
        || program_mode == PROGRAM_MODE_PRINT_CONFIG_HELP
        || program_mode == PROGRAM_MODE_PRINT_ENCRYPTION_HELP
        || program_mode == PROGRAM_MODE_PRINT_LEDGER_HELP
        || program_mode == PROGRAM_MODE_PRINT_NETWORK_HELP
        || program_mode == PROGRAM_MODE_PRINT_OFFERINGS_HELP
        || program_mode == PROGRAM_MODE_PRINT_TALK_HELP
        || program_mode == PROGRAM_MODE_PRINT_SHARE_HELP
        || program_mode == PROGRAM_MODE_PRINT_CREDIT_HELP;
}

static int print_dry_run(
    const struct program_arguments *program_arguments,
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("print_dry_run(): now we delegate dry-run output to the command's module");

    if (program_arguments->program_mode == PROGRAM_MODE_RUN_SERVER) {
        return network_application_print_run_server_dry_run(
            program_arguments->listen_port,
            program_arguments->peer_port,
            resolved_storage_directory_path
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_START_HOME) {
        return files_application_print_start_dry_run(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_HOME) {
        return files_application_print_home_dry_run(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_CREATE_ENCRYPTION_KEYS) {
        return encryption_application_print_create_keys_dry_run(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_GET_CONFIG_VALUE) {
        return config_application_print_get_dry_run(
            resolved_storage_directory_path,
            program_arguments->config_key_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_SET_CONFIG_VALUE) {
        return config_application_print_set_dry_run(
            resolved_storage_directory_path,
            program_arguments->config_key_text,
            program_arguments->config_value_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ADD_CONFIG_VALUE) {
        return config_application_print_add_dry_run(
            resolved_storage_directory_path,
            program_arguments->config_key_text,
            program_arguments->config_value_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_CONFIG_VALUE) {
        return config_application_print_remove_dry_run(
            resolved_storage_directory_path,
            program_arguments->config_key_text,
            program_arguments->config_value_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_RECREATE_ENCRYPTION_KEYS) {
        return encryption_application_print_recreate_keys_dry_run(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ENCRYPT_MESSAGE) {
        return encryption_application_print_encrypt_message_dry_run(program_arguments->message_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_SIGN_MESSAGE) {
        return encryption_application_print_sign_message_dry_run(program_arguments->message_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_LEDGER_SUMMARY) {
        return ledger_application_print_summary_dry_run(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PING_NETWORK_PEER) {
        return network_application_print_ping_dry_run(program_arguments->network_address_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_REMOTE_OFFERINGS) {
        return network_application_print_remote_offerings_dry_run(program_arguments->network_address_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_TALK_PRINT_REMOTE_OFFERINGS) {
        return network_application_print_connected_offerings_dry_run(program_arguments->local_client_port);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_TALK_SEND_MESSAGE) {
        return network_application_print_send_message_dry_run(
            program_arguments->local_client_port,
            program_arguments->message_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_LOCAL_OFFERINGS) {
        return offerings_application_print_local_dry_run(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ADD_OFFERING) {
        return offerings_application_print_add_dry_run(program_arguments->offering_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_EDIT_OFFERING) {
        return offerings_application_print_edit_dry_run(program_arguments->offering_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_OFFERING) {
        return offerings_application_print_remove_dry_run(program_arguments->offering_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_LIST_LOCAL_SHARED_STORAGE) {
        return shared_storage_application_print_local_list_dry_run();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_LIST_REMOTE_SHARED_STORAGE) {
        return shared_storage_application_print_remote_list_dry_run();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ADD_CREDIT_WITHDRAW_CODE) {
        return credit_withdraw_application_print_add_dry_run(
            resolved_storage_directory_path,
            program_arguments->credit_count,
            program_arguments->credit_code_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_CREDIT_WITHDRAW_CODE) {
        return credit_withdraw_application_print_remove_dry_run(
            resolved_storage_directory_path,
            program_arguments->credit_code_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_LIST_CREDIT_WITHDRAW_CODES) {
        return credit_withdraw_application_print_list_dry_run(resolved_storage_directory_path);
    }

    return TALKSPHERE_SUCCESS;
}

/*
 * Runs the specified command based on the parsed program arguments.
 * Here we dispatch the command to the appropriate module application for execution.
 * Mostly, debugging starts here.
 */
static int run_command(
    const struct program_arguments *program_arguments,
    const char *resolved_storage_directory_path
) {
    LOG_TRACE(">run_command(): now we dispatch the parsed command to the owning module application");

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_HOME) {
        return files_application_print_home(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_START_HOME) {
        LOG_TRACE("<run_command(): home files were prepared so start can finish without running networking");
        return TALKSPHERE_SUCCESS;
    }

    if (program_arguments->program_mode == PROGRAM_MODE_CREATE_ENCRYPTION_KEYS) {
        return encryption_application_create_keys(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_GET_CONFIG_VALUE) {
        return config_application_get_value(
            resolved_storage_directory_path,
            program_arguments->config_key_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_SET_CONFIG_VALUE) {
        return config_application_set_value(
            resolved_storage_directory_path,
            program_arguments->config_key_text,
            program_arguments->config_value_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ADD_CONFIG_VALUE) {
        return config_application_add_value(
            resolved_storage_directory_path,
            program_arguments->config_key_text,
            program_arguments->config_value_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_CONFIG_VALUE) {
        return config_application_remove_value(
            resolved_storage_directory_path,
            program_arguments->config_key_text,
            program_arguments->config_value_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_RECREATE_ENCRYPTION_KEYS) {
        return encryption_application_recreate_keys(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ENCRYPT_MESSAGE) {
        return encryption_application_print_encrypted_message(program_arguments->message_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_SIGN_MESSAGE) {
        return encryption_application_print_message_signature(program_arguments->message_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_LEDGER_SUMMARY) {
        return ledger_application_print_local_summary(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_LOCAL_OFFERINGS) {
        return offerings_application_print_local(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_RUN_SERVER) {
        return network_application_run_server(
            program_arguments->listen_port,
            program_arguments->peer_port,
            resolved_storage_directory_path
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PING_NETWORK_PEER) {
        return network_application_print_ping_placeholder();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_REMOTE_OFFERINGS) {
        return network_application_print_remote_offerings_placeholder();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_TALK_PRINT_REMOTE_OFFERINGS) {
        return network_application_print_connected_offerings(program_arguments->local_client_port);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_TALK_SEND_MESSAGE) {
        return network_application_send_connected_message(
            program_arguments->local_client_port,
            program_arguments->message_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ADD_OFFERING) {
        return offerings_application_print_add_placeholder();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_EDIT_OFFERING) {
        return offerings_application_print_edit_placeholder();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_OFFERING) {
        return offerings_application_print_remove_placeholder();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_LIST_LOCAL_SHARED_STORAGE) {
        return shared_storage_application_print_local_list_placeholder();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_LIST_REMOTE_SHARED_STORAGE) {
        return shared_storage_application_print_remote_list_placeholder();
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ADD_CREDIT_WITHDRAW_CODE) {
        return credit_withdraw_application_add_code(
            resolved_storage_directory_path,
            program_arguments->credit_count,
            program_arguments->credit_code_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_CREDIT_WITHDRAW_CODE) {
        return credit_withdraw_application_remove_code(
            resolved_storage_directory_path,
            program_arguments->credit_code_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_LIST_CREDIT_WITHDRAW_CODES) {
        return credit_withdraw_application_list_codes(resolved_storage_directory_path);
    }

    return TALKSPHERE_SUCCESS;
}

int run_talksphere_application(
    int argument_count,
    char *argument_values[]
) {
    LOG_TRACE(">run_talksphere_application(): now we parse startup input and delegate the selected command");
    LOG_DEBUG(
        "Received %d program arguments",
        argument_count
    );

    struct program_arguments program_arguments;
    if (parse_program_arguments(
            argument_count,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        LOG_TRACE("<run_talksphere_application(): command failed");
        return TALKSPHERE_FAILURE;
    }

    if (program_mode_is_help(program_arguments.program_mode)) {
        LOG_TRACE("<run_talksphere_application(): command failed");
        return TALKSPHERE_SUCCESS;
    }

    char resolved_storage_directory_path[PATH_MAX];
    if (resolve_app_storage_directory_path(
            program_arguments.app_storage_directory_path,
            resolved_storage_directory_path,
            sizeof(resolved_storage_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.dry_run_is_enabled) {
        return print_dry_run(
            &program_arguments,
            resolved_storage_directory_path
        );
    }

    if (command_needs_app_files(program_arguments.program_mode)
        && ensure_app_files(resolved_storage_directory_path) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return run_command(
        &program_arguments,
        resolved_storage_directory_path
    );
}
