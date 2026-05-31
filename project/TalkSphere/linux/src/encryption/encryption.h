#ifndef TALKSPHERE_ENCRYPTION_H
#define TALKSPHERE_ENCRYPTION_H

#include <stddef.h>

int create_encryption_keys(
    unsigned char *public_key_bytes,
    size_t public_key_capacity,
    size_t *public_key_size,
    unsigned char *private_key_bytes,
    size_t private_key_capacity,
    size_t *private_key_size
);

int encrypt_message(
    const unsigned char *public_key_bytes,
    size_t public_key_size,
    const unsigned char *message_bytes,
    size_t message_size,
    unsigned char *encrypted_message_bytes,
    size_t encrypted_message_capacity,
    size_t *encrypted_message_size
);

int sign_message(
    const unsigned char *private_key_bytes,
    size_t private_key_size,
    const unsigned char *message_bytes,
    size_t message_size,
    unsigned char *signature_bytes,
    size_t signature_capacity,
    size_t *signature_size
);

#endif
