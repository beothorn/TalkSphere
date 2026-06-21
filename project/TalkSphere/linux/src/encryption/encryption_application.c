#include "encryption_application.h"

#include "encryption.h"

#include "../common/result.h"
#include "../logging.h"

#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#ifndef PATH_MAX
#define PATH_MAX 4096
#endif

#define RECREATE_KEYS 1
#define CREATE_KEYS 0
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

int encryption_application_create_keys(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("encryption_application_create_keys(): now we run the encryption key creation command");

    return create_or_recreate_encryption_keys(
        resolved_storage_directory_path,
        CREATE_KEYS
    );
}

int encryption_application_recreate_keys(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("encryption_application_recreate_keys(): now we run the encryption key recreation command");

    return create_or_recreate_encryption_keys(
        resolved_storage_directory_path,
        RECREATE_KEYS
    );
}

int encryption_application_print_encrypted_message(
    const char *message_text
) {
    LOG_TRACE("encryption_application_print_encrypted_message(): now we call the encryption boundary and print its output");

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

int encryption_application_print_message_signature(
    const char *message_text
) {
    LOG_TRACE("encryption_application_print_message_signature(): now we call the signing boundary and print its output");

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

int encryption_application_print_create_keys_dry_run(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("encryption_application_print_create_keys_dry_run(): now we describe encryption key creation without changing state");

    printf(
        "Would create encryption keys in %s\n",
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}

int encryption_application_print_recreate_keys_dry_run(
    const char *resolved_storage_directory_path
) {
    LOG_TRACE("encryption_application_print_recreate_keys_dry_run(): now we describe encryption key recreation without changing state");

    printf(
        "Would recreate encryption keys in %s\n",
        resolved_storage_directory_path
    );
    return TALKSPHERE_SUCCESS;
}

int encryption_application_print_encrypt_message_dry_run(
    const char *message_text
) {
    LOG_TRACE("encryption_application_print_encrypt_message_dry_run(): now we describe message encryption without changing state");

    printf(
        "Would encrypt message: %s\n",
        message_text
    );
    return TALKSPHERE_SUCCESS;
}

int encryption_application_print_sign_message_dry_run(
    const char *message_text
) {
    LOG_TRACE("encryption_application_print_sign_message_dry_run(): now we describe message signing without changing state");

    printf(
        "Would sign message: %s\n",
        message_text
    );
    return TALKSPHERE_SUCCESS;
}
