#ifndef TALKSPHERE_FILES_APPLICATION_H
#define TALKSPHERE_FILES_APPLICATION_H

int files_application_print_home(
    const char *resolved_storage_directory_path
);

int files_application_print_home_dry_run(
    const char *resolved_storage_directory_path
);

int files_application_print_start_dry_run(
    const char *resolved_storage_directory_path
);

#endif
