#include "logging.h"
#include "argumentParsing/program_arguments.h"
#include "common/result.h"
#include "creditWithdraw/credit_withdraw.h"
#include "encryption/encryption.h"
#include "files/app_files.h"
#include "ledger/ledger_summary.h"
#include "network/socket_channel.h"
#include "offerings/offerings.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

#define IDENTIFIER_TEXT_SIZE 256
#define OFFERINGS_TEXT_SIZE 8192
#define ENCRYPTION_OUTPUT_BUFFER_SIZE 4096
#define ENCRYPTION_PUBLIC_KEY_FILE_NAME "encryption_public.key"
#define ENCRYPTION_PRIVATE_KEY_FILE_NAME "encryption_private.key"
#define ENCRYPTION_KEY_FILE_MODE 0600
#define CREATE_KEY_OPEN_FLAGS (O_WRONLY | O_CREAT | O_EXCL)
#define RECREATE_KEY_OPEN_FLAGS (O_WRONLY | O_TRUNC)
#define FILE_EXISTS 1
#define FILE_DOES_NOT_EXIST 0

static int build_home_file_path(
    const char *home_folder_path,
    const char *file_name,
    char *file_path,
    size_t file_path_size
) {
    LOG_TRACE("build_home_file_path(): now we build a path for a file owned by the app home folder");

    if (snprintf(
            file_path,
            file_path_size,
            "%s/%s",
            home_folder_path,
            file_name
        ) >= (int)file_path_size
    ) {
        LOG_ERROR("The home file path is too long so the command cannot continue");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int write_all_bytes(
    int file_descriptor,
    const unsigned char *file_bytes,
    size_t file_byte_count
) {
    LOG_TRACE("write_all_bytes(): now we write a whole command output buffer to a file");

    size_t written_total_count = 0;
    while (written_total_count < file_byte_count) {
        ssize_t written_byte_count = write(
            file_descriptor,
            file_bytes + written_total_count,
            file_byte_count - written_total_count
        );

        if (written_byte_count <= 0) {
            LOG_ERROR("Writing file bytes failed so the command cannot safely continue");
            return TALKSPHERE_FAILURE;
        }

        written_total_count += (size_t)written_byte_count;
    }

    return TALKSPHERE_SUCCESS;
}

static int write_key_file(
    const char *key_file_path,
    const unsigned char *key_bytes,
    size_t key_byte_count,
    int open_flags
) {
    LOG_TRACE("write_key_file(): now we persist one encryption key file");

    int key_file_descriptor = open(
        key_file_path,
        open_flags,
        ENCRYPTION_KEY_FILE_MODE
    );
    if (key_file_descriptor < 0) {
        if (errno == EEXIST) {
            LOG_WARN("Encryption key creation is unwanted because the key file already exists");
            fprintf(
                stderr,
                "Encryption key already exists: %s\n",
                key_file_path
            );
            return TALKSPHERE_FAILURE;
        }

        if (errno == ENOENT) {
            LOG_WARN("Encryption key recreation is unwanted because the key file does not exist");
            fprintf(
                stderr,
                "Encryption key does not exist: %s\n",
                key_file_path
            );
            return TALKSPHERE_FAILURE;
        }

        LOG_ERROR("Opening the encryption key file failed so key storage cannot continue");
        return TALKSPHERE_FAILURE;
    }

    int write_result = write_all_bytes(
        key_file_descriptor,
        key_bytes,
        key_byte_count
    );

    close(key_file_descriptor);
    return write_result;
}

static int key_file_exists(
    const char *key_file_path,
    int *file_exists
) {
    LOG_TRACE("key_file_exists(): now we check whether a key file exists before mutating key storage");

    if (access(
            key_file_path,
            F_OK
        ) == 0
    ) {
        *file_exists = FILE_EXISTS;
        return TALKSPHERE_SUCCESS;
    }

    if (errno == ENOENT) {
        *file_exists = FILE_DOES_NOT_EXIST;
        return TALKSPHERE_SUCCESS;
    }

    LOG_ERROR("Checking the key file failed so key storage cannot continue safely");
    return TALKSPHERE_FAILURE;
}

static int validate_key_file_state_before_writing(
    const char *public_key_file_path,
    const char *private_key_file_path,
    int recreate_keys
) {
    LOG_TRACE("validate_key_file_state_before_writing(): now we make sure key writing will not partially mutate files");

    int public_key_file_exists = FILE_DOES_NOT_EXIST;
    int private_key_file_exists = FILE_DOES_NOT_EXIST;
    if (key_file_exists(
            public_key_file_path,
            &public_key_file_exists
        ) != TALKSPHERE_SUCCESS
        || key_file_exists(
            private_key_file_path,
            &private_key_file_exists
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (!recreate_keys
        && (public_key_file_exists || private_key_file_exists)
    ) {
        LOG_WARN("Encryption key creation is unwanted because at least one key file already exists");
        fprintf(
            stderr,
            "Encryption keys already exist in the home folder.\n"
        );
        return TALKSPHERE_FAILURE;
    }

    if (recreate_keys
        && (!public_key_file_exists || !private_key_file_exists)
    ) {
        LOG_WARN("Encryption key recreation is unwanted because both key files must already exist");
        fprintf(
            stderr,
            "Encryption keys do not exist in the home folder.\n"
        );
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int command_needs_app_files(
    enum program_mode program_mode
) {
    LOG_TRACE("command_needs_app_files(): now we decide whether the command needs home files before it runs");

    return program_mode == PROGRAM_MODE_RUN_SERVER
        || program_mode == PROGRAM_MODE_PRINT_LEDGER_SUMMARY
        || program_mode == PROGRAM_MODE_CREATE_ENCRYPTION_KEYS
        || program_mode == PROGRAM_MODE_RECREATE_ENCRYPTION_KEYS
        || program_mode == PROGRAM_MODE_ENCRYPT_MESSAGE
        || program_mode == PROGRAM_MODE_SIGN_MESSAGE
        || program_mode == PROGRAM_MODE_PRINT_LOCAL_OFFERINGS
        || program_mode == PROGRAM_MODE_ADD_CREDIT_WITHDRAW_CODE
        || program_mode == PROGRAM_MODE_REMOVE_CREDIT_WITHDRAW_CODE;
}

static int print_dry_run(
    const struct program_arguments *program_arguments,
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("print_dry_run(): now we describe the command without changing local state");

    if (program_arguments->program_mode == PROGRAM_MODE_RUN_SERVER) {
        printf(
            "Would run TalkSphere with listen port %d, peer port %d, home folder %s\n",
            program_arguments->listen_port,
            program_arguments->peer_port,
            resolved_storage_directory_path
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_PRINT_HOME) {
        printf(
            "Would print home folder %s\n",
            resolved_storage_directory_path
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_CREATE_ENCRYPTION_KEYS) {
        printf(
            "Would create encryption keys in %s\n",
            resolved_storage_directory_path
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_RECREATE_ENCRYPTION_KEYS) {
        printf(
            "Would recreate encryption keys in %s\n",
            resolved_storage_directory_path
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_ENCRYPT_MESSAGE) {
        printf(
            "Would encrypt message: %s\n",
            program_arguments->message_text
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_SIGN_MESSAGE) {
        printf(
            "Would sign message: %s\n",
            program_arguments->message_text
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_PRINT_LEDGER_SUMMARY) {
        printf(
            "Would print ledger credit summary from %s\n",
            resolved_storage_directory_path
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_PING_NETWORK_PEER) {
        printf(
            "Would ping TalkSphere peer %s\n",
            program_arguments->network_address_text
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_PRINT_REMOTE_OFFERINGS) {
        printf(
            "Would print offerings from peer %s\n",
            program_arguments->network_address_text
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_TALK_PRINT_REMOTE_OFFERINGS) {
        printf(
            "Would ask local TalkSphere client port %d for connected peer offerings\n",
            program_arguments->local_client_port
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_TALK_SEND_MESSAGE) {
        printf(
            "Would ask local TalkSphere client port %d to send message: %s\n",
            program_arguments->local_client_port,
            program_arguments->message_text
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_PRINT_LOCAL_OFFERINGS) {
        printf(
            "Would print local offerings from %s\n",
            resolved_storage_directory_path
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_ADD_OFFERING) {
        printf(
            "Would add offering: %s\n",
            program_arguments->offering_text
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_EDIT_OFFERING) {
        printf(
            "Would edit offering: %s\n",
            program_arguments->offering_text
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_OFFERING) {
        printf(
            "Would remove offering: %s\n",
            program_arguments->offering_text
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_LIST_LOCAL_SHARED_STORAGE) {
        printf("Would list local shared storage metadata\n");
    } else if (program_arguments->program_mode == PROGRAM_MODE_LIST_REMOTE_SHARED_STORAGE) {
        printf("Would list remote shared storage metadata\n");
    } else if (program_arguments->program_mode == PROGRAM_MODE_ADD_CREDIT_WITHDRAW_CODE) {
        printf(
            "Would add %d credit withdraw code %s in %s\n",
            program_arguments->credit_count,
            program_arguments->credit_code_text,
            resolved_storage_directory_path
        );
    } else if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_CREDIT_WITHDRAW_CODE) {
        printf(
            "Would remove credit withdraw code %s from %s\n",
            program_arguments->credit_code_text,
            resolved_storage_directory_path
        );
    }

    return TALKSPHERE_SUCCESS;
}

static int create_or_recreate_encryption_keys(
    const char *resolved_storage_directory_path,
    int recreate_keys
) {
    LOG_TRACE("create_or_recreate_encryption_keys(): now we create the current placeholder key files");

    unsigned char public_key_bytes[ENCRYPTION_OUTPUT_BUFFER_SIZE];
    unsigned char private_key_bytes[ENCRYPTION_OUTPUT_BUFFER_SIZE];
    size_t public_key_size = 0;
    size_t private_key_size = 0;

    if (create_encryption_keys(
            public_key_bytes,
            sizeof(public_key_bytes),
            &public_key_size,
            private_key_bytes,
            sizeof(private_key_bytes),
            &private_key_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char public_key_file_path[PATH_MAX];
    char private_key_file_path[PATH_MAX];
    if (build_home_file_path(
            resolved_storage_directory_path,
            ENCRYPTION_PUBLIC_KEY_FILE_NAME,
            public_key_file_path,
            sizeof(public_key_file_path)
        ) != TALKSPHERE_SUCCESS
        || build_home_file_path(
            resolved_storage_directory_path,
            ENCRYPTION_PRIVATE_KEY_FILE_NAME,
            private_key_file_path,
            sizeof(private_key_file_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    int open_flags = recreate_keys
        ? RECREATE_KEY_OPEN_FLAGS
        : CREATE_KEY_OPEN_FLAGS;

    if (validate_key_file_state_before_writing(
            public_key_file_path,
            private_key_file_path,
            recreate_keys
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (write_key_file(
            public_key_file_path,
            public_key_bytes,
            public_key_size,
            open_flags
        ) != TALKSPHERE_SUCCESS
        || write_key_file(
            private_key_file_path,
            private_key_bytes,
            private_key_size,
            open_flags
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Encryption keys stored in %s\n",
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}

static int print_encrypted_message(
    const char *message_text
) {
    LOG_TRACE("print_encrypted_message(): now we call the encryption boundary and print its output");

    unsigned char encrypted_message_bytes[ENCRYPTION_OUTPUT_BUFFER_SIZE];
    size_t encrypted_message_size = 0;
    const unsigned char public_key_bytes[] = "";

    if (encrypt_message(
            public_key_bytes,
            sizeof(public_key_bytes),
            (const unsigned char *)message_text,
            strlen(message_text),
            encrypted_message_bytes,
            sizeof(encrypted_message_bytes),
            &encrypted_message_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    fwrite(
        encrypted_message_bytes,
        sizeof(unsigned char),
        encrypted_message_size,
        stdout
    );
    printf("\n");
    return TALKSPHERE_SUCCESS;
}

static int print_message_signature(
    const char *message_text
) {
    LOG_TRACE("print_message_signature(): now we call the signing boundary and print its output");

    unsigned char signature_bytes[ENCRYPTION_OUTPUT_BUFFER_SIZE];
    size_t signature_size = 0;
    const unsigned char private_key_bytes[] = "";

    if (sign_message(
            private_key_bytes,
            sizeof(private_key_bytes),
            (const unsigned char *)message_text,
            strlen(message_text),
            signature_bytes,
            sizeof(signature_bytes),
            &signature_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    fwrite(
        signature_bytes,
        sizeof(unsigned char),
        signature_size,
        stdout
    );
    printf("\n");
    return TALKSPHERE_SUCCESS;
}

static int print_local_offerings(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("print_local_offerings(): now we print the local offerings document");

    char offerings_text[OFFERINGS_TEXT_SIZE];
    if (read_local_offerings(
            resolved_storage_directory_path,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "%s",
        offerings_text
    );
    return TALKSPHERE_SUCCESS;
}

static int print_connected_talksphere_offerings(
    int local_client_port
) {
    LOG_TRACE("print_connected_talksphere_offerings(): now we ask a running local instance to fetch its connected peer offerings");

    char offerings_text[OFFERINGS_TEXT_SIZE];
    if (request_remote_offerings_through_client_port(
            local_client_port,
            offerings_text,
            sizeof(offerings_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "%s",
        offerings_text
    );
    return TALKSPHERE_SUCCESS;
}

static int send_connected_talksphere_message(
    int local_client_port,
    const char *message_text
) {
    LOG_TRACE("send_connected_talksphere_message(): now we ask a running local instance to send a message to its connected peer");

    return request_message_send_through_client_port(
        local_client_port,
        message_text
    );
}

static int add_credit_withdraw_code(
    const char *resolved_storage_directory_path,
    int credit_count,
    const char *credit_code_text
) {
    LOG_TRACE("add_credit_withdraw_code(): now we store a credit withdraw code for the local id");

    char local_identifier_text[IDENTIFIER_TEXT_SIZE];
    if (read_local_identifier(
            resolved_storage_directory_path,
            local_identifier_text,
            sizeof(local_identifier_text)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (credit_withdraw_add_code(
            resolved_storage_directory_path,
            local_identifier_text,
            credit_count,
            credit_code_text
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Stored %d credit for code %s\n",
        credit_count,
        credit_code_text
    );
    return TALKSPHERE_SUCCESS;
}

static int remove_credit_withdraw_code(
    const char *resolved_storage_directory_path,
    const char *credit_code_text
) {
    LOG_TRACE("remove_credit_withdraw_code(): now we remove a credit withdraw code from local storage");

    if (credit_withdraw_remove_code(
            resolved_storage_directory_path,
            credit_code_text
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Removed credit code %s\n",
        credit_code_text
    );
    return TALKSPHERE_SUCCESS;
}

static int print_placeholder(
    const char *placeholder_text
) {
    LOG_TRACE("print_placeholder(): now we report a command that exists before the feature body is implemented");

    printf(
        "%s\n",
        placeholder_text
    );
    return TALKSPHERE_SUCCESS;
}

static int run_command(
    const struct program_arguments *program_arguments,
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("run_command(): now we dispatch the parsed command to the owning module or placeholder");

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_HOME) {
        printf(
            "%s\n",
            resolved_storage_directory_path
        );
        return TALKSPHERE_SUCCESS;
    }

    if (program_arguments->program_mode == PROGRAM_MODE_CREATE_ENCRYPTION_KEYS) {
        return create_or_recreate_encryption_keys(
            resolved_storage_directory_path,
            0
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_RECREATE_ENCRYPTION_KEYS) {
        return create_or_recreate_encryption_keys(
            resolved_storage_directory_path,
            1
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ENCRYPT_MESSAGE) {
        return print_encrypted_message(program_arguments->message_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_SIGN_MESSAGE) {
        return print_message_signature(program_arguments->message_text);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_LEDGER_SUMMARY) {
        char local_identifier_text[IDENTIFIER_TEXT_SIZE];
        if (read_local_identifier(
                resolved_storage_directory_path,
                local_identifier_text,
                sizeof(local_identifier_text)
            ) != TALKSPHERE_SUCCESS
        ) {
            return TALKSPHERE_FAILURE;
        }

        return print_ledger_summary(
            resolved_storage_directory_path,
            local_identifier_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_LOCAL_OFFERINGS) {
        return print_local_offerings(resolved_storage_directory_path);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_RUN_SERVER) {
        return run_socket_channel(
            program_arguments->listen_port,
            program_arguments->peer_port,
            resolved_storage_directory_path
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PING_NETWORK_PEER) {
        return print_placeholder("network ping is not implemented yet");
    }

    if (program_arguments->program_mode == PROGRAM_MODE_PRINT_REMOTE_OFFERINGS) {
        return print_placeholder("remote offerings lookup is not implemented yet");
    }

    if (program_arguments->program_mode == PROGRAM_MODE_TALK_PRINT_REMOTE_OFFERINGS) {
        return print_connected_talksphere_offerings(program_arguments->local_client_port);
    }

    if (program_arguments->program_mode == PROGRAM_MODE_TALK_SEND_MESSAGE) {
        return send_connected_talksphere_message(
            program_arguments->local_client_port,
            program_arguments->message_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ADD_OFFERING) {
        return print_placeholder("offering add is not implemented yet");
    }

    if (program_arguments->program_mode == PROGRAM_MODE_EDIT_OFFERING) {
        return print_placeholder("offering edit is not implemented yet");
    }

    if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_OFFERING) {
        return print_placeholder("offering remove is not implemented yet");
    }

    if (program_arguments->program_mode == PROGRAM_MODE_LIST_LOCAL_SHARED_STORAGE) {
        return print_placeholder("local shared storage listing is not implemented yet");
    }

    if (program_arguments->program_mode == PROGRAM_MODE_LIST_REMOTE_SHARED_STORAGE) {
        return print_placeholder("remote shared storage listing is not implemented yet");
    }

    if (program_arguments->program_mode == PROGRAM_MODE_ADD_CREDIT_WITHDRAW_CODE) {
        return add_credit_withdraw_code(
            resolved_storage_directory_path,
            program_arguments->credit_count,
            program_arguments->credit_code_text
        );
    }

    if (program_arguments->program_mode == PROGRAM_MODE_REMOVE_CREDIT_WITHDRAW_CODE) {
        return remove_credit_withdraw_code(
            resolved_storage_directory_path,
            program_arguments->credit_code_text
        );
    }

    return TALKSPHERE_SUCCESS;
}

int main(
    int argument_count,
    char *argument_values[]
) {
    LOG_TRACE("main(): starting the program entrypoint");

    struct program_arguments program_arguments;
    if (parse_program_arguments(
            argument_count,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.program_mode == PROGRAM_MODE_PRINT_MAIN_HELP
        || program_arguments.program_mode == PROGRAM_MODE_PRINT_CONFIG_HELP
        || program_arguments.program_mode == PROGRAM_MODE_PRINT_ENCRYPTION_HELP
        || program_arguments.program_mode == PROGRAM_MODE_PRINT_LEDGER_HELP
        || program_arguments.program_mode == PROGRAM_MODE_PRINT_NETWORK_HELP
        || program_arguments.program_mode == PROGRAM_MODE_PRINT_OFFERINGS_HELP
        || program_arguments.program_mode == PROGRAM_MODE_PRINT_SHARE_HELP
        || program_arguments.program_mode == PROGRAM_MODE_PRINT_CREDIT_HELP
    ) {
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
