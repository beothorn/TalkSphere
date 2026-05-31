#ifndef TALKSPHERE_LEDGER_H
#define TALKSPHERE_LEDGER_H

int ledger_add_credit(
    const char *app_storage_directory_path,
    const char *identifier_text
);

int ledger_spend_credit(
    const char *app_storage_directory_path,
    const char *identifier_text
);

int ledger_get_credits(
    const char *app_storage_directory_path,
    const char *identifier_text,
    int *credit_count
);

#endif
