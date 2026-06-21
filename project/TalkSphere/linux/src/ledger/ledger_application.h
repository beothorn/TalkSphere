#ifndef TALKSPHERE_LEDGER_APPLICATION_H
#define TALKSPHERE_LEDGER_APPLICATION_H

int ledger_application_print_local_summary(
    const char *resolved_storage_directory_path
);

int ledger_application_print_summary_dry_run(
    const char *resolved_storage_directory_path
);

#endif
