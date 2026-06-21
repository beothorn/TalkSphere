#ifndef TALKSPHERE_CREDIT_WITHDRAW_APPLICATION_H
#define TALKSPHERE_CREDIT_WITHDRAW_APPLICATION_H

int credit_withdraw_application_add_code(
    const char *resolved_storage_directory_path,
    int credit_count,
    const char *credit_code_text
);

int credit_withdraw_application_remove_code(
    const char *resolved_storage_directory_path,
    const char *credit_code_text
);

int credit_withdraw_application_list_codes(
    const char *resolved_storage_directory_path
);

int credit_withdraw_application_print_add_dry_run(
    const char *resolved_storage_directory_path,
    int credit_count,
    const char *credit_code_text
);

int credit_withdraw_application_print_remove_dry_run(
    const char *resolved_storage_directory_path,
    const char *credit_code_text
);

int credit_withdraw_application_print_list_dry_run(
    const char *resolved_storage_directory_path
);

#endif
