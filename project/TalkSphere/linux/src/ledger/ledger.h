#ifndef TALKSPHERE_LEDGER_H
#define TALKSPHERE_LEDGER_H

struct ledger_credit_summary {
    int owned_credits;
    int owed_credits;
};

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

int ledger_get_credit_summary(
    const char *app_storage_directory_path,
    const char *local_identifier_text,
    struct ledger_credit_summary *ledger_credit_summary
);

#endif
