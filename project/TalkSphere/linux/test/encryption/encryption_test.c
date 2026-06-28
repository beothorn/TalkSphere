#include "encryption/encryption.h"
#include "common/result.h"
#include "test_support.h"

#include <stddef.h>

static int test_create_encrypt_and_sign_placeholders(void) {
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
        return TEST_FAILURE;
    }

    if (public_key_size != 0 || private_key_size != 0) {
        return TEST_FAILURE;
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
        return TEST_FAILURE;
    }

    if (encrypted_message_size != 0) {
        return TEST_FAILURE;
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
        return TEST_FAILURE;
    }

    if (signature_size != 0) {
        return TEST_FAILURE;
    }

    return TEST_SUCCESS;
}

static int test_rejects_missing_output_sizes(void) {
    unsigned char public_key_bytes[1];
    unsigned char private_key_bytes[1];
    unsigned char encrypted_message_bytes[1];
    unsigned char signature_bytes[1];
    const unsigned char message_bytes[] = "hello";
    size_t public_key_size = 0;
    size_t private_key_size = 0;

    if (create_encryption_keys(
            public_key_bytes,
            sizeof(public_key_bytes),
            NULL,
            private_key_bytes,
            sizeof(private_key_bytes),
            &private_key_size
        ) != TALKSPHERE_FAILURE
    ) {
        return TEST_FAILURE;
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
        return TEST_FAILURE;
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
        return TEST_FAILURE;
    }

    return TEST_SUCCESS;
}

int main(void) {
    const struct test_case test_cases[] = {
        TEST_CASE(test_create_encrypt_and_sign_placeholders),
        TEST_CASE(test_rejects_missing_output_sizes)
    };

    return run_test_cases(
        test_cases,
        sizeof(test_cases) / sizeof(test_cases[0])
    );
}
