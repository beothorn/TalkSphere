#include "encryption.h"

#include "../common/result.h"
#include "../logging.h"

#define OUTPUT_BUFFER_IS_AVAILABLE 0
#define OUTPUT_BUFFER_IS_MISSING 1

static int output_buffer_is_missing(
    unsigned char *buffer_bytes,
    size_t buffer_capacity,
    size_t *buffer_size
) {
    LOG_TRACE("output_buffer_is_missing(): now we check whether a placeholder output buffer can receive a result");

    if (buffer_size == NULL) {
        LOG_WARN("The output size pointer is unwanted because callers need to know how many bytes were written");
        return OUTPUT_BUFFER_IS_MISSING;
    }

    if (buffer_bytes == NULL && buffer_capacity > 0) {
        LOG_WARN("The output buffer is unwanted because a positive capacity without storage cannot receive bytes");
        return OUTPUT_BUFFER_IS_MISSING;
    }

    return OUTPUT_BUFFER_IS_AVAILABLE;
}

int create_encryption_keys(
    unsigned char *public_key_bytes,
    size_t public_key_capacity,
    size_t *public_key_size,
    unsigned char *private_key_bytes,
    size_t private_key_capacity,
    size_t *private_key_size
) {
    LOG_TRACE("create_encryption_keys(): now we reserve the key creation boundary before real crypto is implemented");

    if (output_buffer_is_missing(
            public_key_bytes,
            public_key_capacity,
            public_key_size
        )
        || output_buffer_is_missing(
            private_key_bytes,
            private_key_capacity,
            private_key_size
        )
    ) {
        return TALKSPHERE_FAILURE;
    }

    *public_key_size = 0;
    *private_key_size = 0;

    (void)public_key_bytes;
    (void)public_key_capacity;
    (void)private_key_bytes;
    (void)private_key_capacity;

    return TALKSPHERE_SUCCESS;
}

int encrypt_message(
    const unsigned char *public_key_bytes,
    size_t public_key_size,
    const unsigned char *message_bytes,
    size_t message_size,
    unsigned char *encrypted_message_bytes,
    size_t encrypted_message_capacity,
    size_t *encrypted_message_size
) {
    LOG_TRACE("encrypt_message(): now we reserve the message encryption boundary before real crypto is implemented");

    if (output_buffer_is_missing(
            encrypted_message_bytes,
            encrypted_message_capacity,
            encrypted_message_size
        )
    ) {
        return TALKSPHERE_FAILURE;
    }

    *encrypted_message_size = 0;

    (void)public_key_bytes;
    (void)public_key_size;
    (void)message_bytes;
    (void)message_size;
    (void)encrypted_message_bytes;
    (void)encrypted_message_capacity;

    return TALKSPHERE_SUCCESS;
}

int sign_message(
    const unsigned char *private_key_bytes,
    size_t private_key_size,
    const unsigned char *message_bytes,
    size_t message_size,
    unsigned char *signature_bytes,
    size_t signature_capacity,
    size_t *signature_size
) {
    LOG_TRACE("sign_message(): now we reserve the message signing boundary before real crypto is implemented");

    if (output_buffer_is_missing(
            signature_bytes,
            signature_capacity,
            signature_size
        )
    ) {
        return TALKSPHERE_FAILURE;
    }

    *signature_size = 0;

    (void)private_key_bytes;
    (void)private_key_size;
    (void)message_bytes;
    (void)message_size;
    (void)signature_bytes;
    (void)signature_capacity;

    return TALKSPHERE_SUCCESS;
}
