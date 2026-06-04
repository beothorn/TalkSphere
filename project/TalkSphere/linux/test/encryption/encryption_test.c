#include "encryption/encryption.h"
#include "common/result.h"

#include <stddef.h>

int main(void) {
    unsigned char public_key_bytes[1];
    unsigned char private_key_bytes[1];
    unsigned char encrypted_message_bytes[1];
    unsigned char signature_bytes[1];
    const unsigned char message_bytes[] = "hello";
    size_t public_key_size = 99;
    size_t private_key_size = 99;
    size_t encrypted_message_size = 99;
    size_t signature_size = 99;

    if (create_encryption_keys(
            public_key_bytes,
            sizeof(public_key_bytes),
            &public_key_size,
            private_key_bytes,
            sizeof(private_key_bytes),
            &private_key_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return 1;
    }

    if (public_key_size != 0 || private_key_size != 0) {
        return 2;
    }

    if (encrypt_message(
            public_key_bytes,
            public_key_size,
            message_bytes,
            sizeof(message_bytes),
            encrypted_message_bytes,
            sizeof(encrypted_message_bytes),
            &encrypted_message_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return 3;
    }

    if (encrypted_message_size != 0) {
        return 4;
    }

    if (sign_message(
            private_key_bytes,
            private_key_size,
            message_bytes,
            sizeof(message_bytes),
            signature_bytes,
            sizeof(signature_bytes),
            &signature_size
        ) != TALKSPHERE_SUCCESS
    ) {
        return 5;
    }

    if (signature_size != 0) {
        return 6;
    }

    if (create_encryption_keys(
            public_key_bytes,
            sizeof(public_key_bytes),
            NULL,
            private_key_bytes,
            sizeof(private_key_bytes),
            &private_key_size
        ) != TALKSPHERE_FAILURE
    ) {
        return 7;
    }

    if (encrypt_message(
            public_key_bytes,
            public_key_size,
            message_bytes,
            sizeof(message_bytes),
            encrypted_message_bytes,
            sizeof(encrypted_message_bytes),
            NULL
        ) != TALKSPHERE_FAILURE
    ) {
        return 8;
    }

    if (sign_message(
            private_key_bytes,
            private_key_size,
            message_bytes,
            sizeof(message_bytes),
            signature_bytes,
            sizeof(signature_bytes),
            NULL
        ) != TALKSPHERE_FAILURE
    ) {
        return 9;
    }

    return 0;
}
