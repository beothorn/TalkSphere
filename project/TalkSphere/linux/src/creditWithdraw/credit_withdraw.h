#ifndef TALKSPHERE_CREDIT_WITHDRAW_H
#define TALKSPHERE_CREDIT_WITHDRAW_H

#include <stddef.h>

struct credit_withdraw_entry {
    char owner_identifier_text[256];
    int credit_count;
};

int credit_withdraw_add_code(
    const char *app_storage_directory_path,
    const char *owner_identifier_text,
    int credit_count,
    const char *credit_code_text
);

int credit_withdraw_remove_code(
    const char *app_storage_directory_path,
    const char *credit_code_text
);

int credit_withdraw_find_code(
    const char *app_storage_directory_path,
    const char *credit_code_text,
    struct credit_withdraw_entry *credit_withdraw_entry
);

#endif
