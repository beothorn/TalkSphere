#ifndef TALKSPHERE_OFFERINGS_H
#define TALKSPHERE_OFFERINGS_H

#include <stddef.h>

int read_local_offerings(
    const char *app_storage_directory_path,
    char *offerings_text,
    size_t offerings_text_size
);

#endif
