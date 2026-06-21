#ifndef TALKSPHERE_ENCRYPTION_APPLICATION_H
#define TALKSPHERE_ENCRYPTION_APPLICATION_H

int encryption_application_create_keys(
    const char *resolved_storage_directory_path
);

int encryption_application_recreate_keys(
    const char *resolved_storage_directory_path
);

int encryption_application_print_encrypted_message(
    const char *message_text
);

int encryption_application_print_message_signature(
    const char *message_text
);

int encryption_application_print_create_keys_dry_run(
    const char *resolved_storage_directory_path
);

int encryption_application_print_recreate_keys_dry_run(
    const char *resolved_storage_directory_path
);

int encryption_application_print_encrypt_message_dry_run(
    const char *message_text
);

int encryption_application_print_sign_message_dry_run(
    const char *message_text
);

#endif
