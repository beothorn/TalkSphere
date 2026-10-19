"""Regenerate a test-only Messaging example independently of TalkSphere's Rust code."""
from pathlib import Path
import hashlib
import hmac
import nacl
from nacl.bindings import (
    crypto_scalarmult, crypto_scalarmult_base,
    crypto_aead_xchacha20poly1305_ietf_encrypt,
    crypto_aead_xchacha20poly1305_ietf_decrypt,
)

recipient_private = bytes(range(0x60, 0x80))
ephemeral_private = bytes(range(0x40, 0x60))
recipient_key = crypto_scalarmult_base(recipient_private)
ephemeral_key = crypto_scalarmult_base(ephemeral_private)
recipient_id = bytes.fromhex('774504edf47bae28a4f1154bf4516ae74f090ee5b032464586d0c21d24d9b249')
sender_id = bytes.fromhex('d948a9a847999305be609db39d9ca50b7c93184ec36bdf080f6f46415e19925d')
message_id = bytes(range(32))
metadata = (b'CR2SE-MESSAGE-ENCRYPTION-METADATA-V1' + message_id + sender_id
            + recipient_id + (1000).to_bytes(8, 'big'))
nonce = bytes(range(0x80, 0x98))
plaintext = b'CR2SE identity encryption test'
shared = crypto_scalarmult(ephemeral_private, recipient_key)
salt = hashlib.sha256(metadata).digest()
prk = hmac.digest(salt, shared, 'sha256')
key = hmac.digest(prk, b'CR2SE-MESSAGE-PAYLOAD-KEY-V1' + b'\x01', 'sha256')
sealed = crypto_aead_xchacha20poly1305_ietf_encrypt(plaintext, metadata, nonce, key)
assert crypto_aead_xchacha20poly1305_ietf_decrypt(sealed, metadata, nonce, key) == plaintext
payload = b'\x01' + recipient_key + ephemeral_key + nonce + sealed
text = '''# Independently generated Messaging separation example

Source construction: https://github.com/beothorn/CR2SE/blob/80f6795864ecf060e23482ee42909da4a649ecf0/modules/messaging/README.md

This local fixture follows the published Messaging payload construction; it is
not an upstream-supplied fixed vector. Inputs reuse public test keys from the
Cryptography vector, message ID 00..1f and senderCreatedAt=1000. All private and
derived values are test-only. Regenerate with Python and PyNaCl using the adjacent
script; Rust tests consume literal bytes, not a Rust-generated expected result.
'''
text += f'\nGenerated with PyNaCl {nacl.__version__}, Python hashlib/HMAC and libsodium AEAD.\n\n```text\n'
for label, value in [('metadata', metadata), ('nonce', nonce), ('derived key', key),
                     ('plaintext', plaintext), ('sealed', sealed), ('payload', payload)]:
    text += f'{label}:\n{value.hex()}\n\n'
text += '```\n'
Path(__file__).with_name('messaging.md').write_text(text)
