#ifndef TALKSPHERE_CREDIT_WITHDRAW_H
#define TALKSPHERE_CREDIT_WITHDRAW_H

#include <stddef.h>

struct credit_withdraw_entry {
    char credit_code_text[256];
    char owner_identifier_text[256];
    int credit_count;
};

typedef int (*credit_withdraw_entry_callback)(
    const struct credit_withdraw_entry *credit_withdraw_entry,
    void *callback_context
);

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

int credit_withdraw_list_codes(
    const char *app_storage_directory_path,
    credit_withdraw_entry_callback entry_callback,
    void *callback_context
);

#endif
