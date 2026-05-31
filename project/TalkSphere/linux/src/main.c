#include "logging.h"
#include "argumentParsing/program_arguments.h"
#include "files/app_files.h"
#include "ledger/ledger_summary.h"
#include "network/socket_channel.h"

#include <limits.h>

#define IDENTIFIER_TEXT_SIZE 256

int main(
    int argument_count,
    char *argument_values[]
) {
    LOG_TRACE("main(): starting the program entrypoint");

    struct program_arguments program_arguments;
    if (parse_program_arguments(
            argument_count,
            argument_values,
            &program_arguments
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    char resolved_storage_directory_path[PATH_MAX];
    if (resolve_app_storage_directory_path(
            program_arguments.app_storage_directory_path,
            resolved_storage_directory_path,
            sizeof(resolved_storage_directory_path)
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (ensure_app_files(resolved_storage_directory_path) != TALKSPHERE_SUCCESS) {
        return TALKSPHERE_FAILURE;
    }

    if (program_arguments.program_mode == PROGRAM_MODE_PRINT_LEDGER_SUMMARY) {
        char local_identifier_text[IDENTIFIER_TEXT_SIZE];
        if (read_local_identifier(
                resolved_storage_directory_path,
                local_identifier_text,
                sizeof(local_identifier_text)
            ) != TALKSPHERE_SUCCESS
        ) {
            return TALKSPHERE_FAILURE;
        }

        return print_ledger_summary(
            resolved_storage_directory_path,
            local_identifier_text
        );
    }

    return run_socket_channel(
        program_arguments.client_port,
        program_arguments.server_port,
        resolved_storage_directory_path
    );
}
