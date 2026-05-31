#include "ledger_summary.h"

#include "ledger.h"
#include "../common/result.h"
#include "../logging.h"

#include <stdio.h>

int print_ledger_summary(
    const char *app_storage_directory_path,
    const char *local_identifier_text
) {
    LOG_TRACE("print_ledger_summary(): now the ledger module prints how many credits the local id owns and owes");

    struct ledger_credit_summary ledger_credit_summary;
    if (ledger_get_credit_summary(
            app_storage_directory_path,
            local_identifier_text,
            &ledger_credit_summary
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    printf(
        "Owned credits: %d\n"
        "Owed credits: %d\n",
        ledger_credit_summary.owned_credits,
        ledger_credit_summary.owed_credits
    );

    return TALKSPHERE_SUCCESS;
}
