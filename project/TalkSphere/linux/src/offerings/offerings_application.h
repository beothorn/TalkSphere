#ifndef TALKSPHERE_OFFERINGS_APPLICATION_H
#define TALKSPHERE_OFFERINGS_APPLICATION_H

int offerings_application_print_local(
    const char *resolved_storage_directory_path
);

int offerings_application_print_local_dry_run(
    const char *resolved_storage_directory_path
);

int offerings_application_print_add_dry_run(
    const char *offering_text
);

int offerings_application_print_edit_dry_run(
    const char *offering_text
);

int offerings_application_print_remove_dry_run(
    const char *offering_text
);

int offerings_application_print_add_placeholder(void);

int offerings_application_print_edit_placeholder(void);

int offerings_application_print_remove_placeholder(void);

#endif
