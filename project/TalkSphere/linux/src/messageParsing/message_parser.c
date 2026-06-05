#include "message_parser.h"

#include "../common/result.h"
#include "../files/app_files.h"
#include "../ledger/ledger.h"
#include "../logging.h"
#include "../offerings/offerings.h"

#include <arpa/inet.h>
#include <stdio.h>
#include <string.h>

#define CONNECT_PREFIX "CONNECT:"
#define MESSAGE_PREFIX "MESSAGE:"
#define PAY_PREFIX "PAY:"
#define CREDITS_PREFIX "CREDITS:"
#define LIST_OFFERINGS_COMMAND "LIST_OFFERINGS"
#define IDENTIFIER_SIZE 256

struct connect_instruction {
    char target_host[INET_ADDRSTRLEN];
    int target_port;
    char reply_host[INET_ADDRSTRLEN];
    int reply_port;
};

static int parse_connect_instruction(
    const char *message_text,
    struct connect_instruction *connect_instruction
) {
    LOG_TRACE("parse_connect_instruction(): now we parse a connect request coming from an external caller");

    int scanned_fields = sscanf(
        message_text,
        "CONNECT:%15[^:]:%d,FROM:%15[^:]:%d",
        connect_instruction->target_host,
        &connect_instruction->target_port,
        connect_instruction->reply_host,
        &connect_instruction->reply_port
    );

    if (scanned_fields != 4) {
        LOG_WARN("The input message is unwanted because it does not match the CONNECT message shape");
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

static int handle_connect_instruction(
    const struct connect_instruction *connect_instruction,
    const struct message_processing_dependencies *message_processing_dependencies
) {
    LOG_TRACE("handle_connect_instruction(): now this server connects to the target and sends a hello to the caller endpoint");

    if (connect_instruction->target_port == message_processing_dependencies->listening_port) {
        LOG_WARN("The target port is unwanted because it would cause a self-loop connection");
        return TALKSPHERE_FAILURE;
    }

    if (message_processing_dependencies->send_message_to_endpoint(
            connect_instruction->target_host,
            connect_instruction->target_port,
            "MESSAGE:Hello"
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    if (message_processing_dependencies->send_message_to_endpoint(
            connect_instruction->reply_host,
            connect_instruction->reply_port,
            "MESSAGE:Hello"
        ) != TALKSPHERE_SUCCESS
    ) {
        return TALKSPHERE_FAILURE;
    }

    return TALKSPHERE_SUCCESS;
}

int process_received_message(
    const char *message_text,
    const struct message_processing_dependencies *message_processing_dependencies,
    char *response_text,
    size_t response_text_size
) {
    LOG_TRACE("process_received_message(): now we branch based on the incoming message type");

    if (response_text != NULL && response_text_size > 0) {
        response_text[0] = '\0';
    }

    if (strcmp(message_text, LIST_OFFERINGS_COMMAND) == 0) {
        LOG_TRACE("process_received_message(): now we return the current local offerings document to the connected peer");

        return read_local_offerings(
            message_processing_dependencies->app_storage_directory_path,
            response_text,
            response_text_size
        );
    }

    if (strncmp(message_text, CONNECT_PREFIX, strlen(CONNECT_PREFIX)) == 0) {
        struct connect_instruction connect_instruction;

        if (parse_connect_instruction(message_text, &connect_instruction) != TALKSPHERE_SUCCESS) {
            return TALKSPHERE_FAILURE;
        }

        return handle_connect_instruction(
            &connect_instruction,
            message_processing_dependencies
        );
    }

    if (strncmp(message_text, PAY_PREFIX, strlen(PAY_PREFIX)) == 0) {
        const char *identifier_text = message_text + strlen(PAY_PREFIX);

        return ledger_add_credit(
            message_processing_dependencies->app_storage_directory_path,
            identifier_text
        );
    }

    if (strncmp(message_text, CREDITS_PREFIX, strlen(CREDITS_PREFIX)) == 0) {
        const char *identifier_text = message_text + strlen(CREDITS_PREFIX);
        int credit_count = 0;

        if (ledger_get_credits(
                message_processing_dependencies->app_storage_directory_path,
                identifier_text,
                &credit_count
            ) != TALKSPHERE_SUCCESS
        ) {
            return TALKSPHERE_FAILURE;
        }

        printf("%d\n", credit_count);
        fflush(stdout);
        return TALKSPHERE_SUCCESS;
    }

    if (strncmp(message_text, MESSAGE_PREFIX, strlen(MESSAGE_PREFIX)) == 0) {
        char local_identifier[IDENTIFIER_SIZE];

        if (read_local_identifier(
                message_processing_dependencies->app_storage_directory_path,
                local_identifier,
                sizeof(local_identifier)
            ) != TALKSPHERE_SUCCESS
        ) {
            return TALKSPHERE_FAILURE;
        }

        if (ledger_spend_credit(
                message_processing_dependencies->app_storage_directory_path,
                local_identifier
            ) != TALKSPHERE_SUCCESS
        ) {
            LOG_WARN("This MESSAGE is unwanted because the local id does not have enough credits");
            return TALKSPHERE_FAILURE;
        }

        printf("%s\n", message_text + strlen(MESSAGE_PREFIX));
        fflush(stdout);
        return TALKSPHERE_SUCCESS;
    }

    LOG_WARN("The message type is unwanted because this server only knows CONNECT, MESSAGE, PAY, CREDITS, and LIST_OFFERINGS");
    return TALKSPHERE_FAILURE;
}
