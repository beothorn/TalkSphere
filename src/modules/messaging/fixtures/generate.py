"""Regenerate vectors.json independently of TalkSphere's Rust code.

Uses PyNaCl (libsodium) for Ed25519 and Python's hashlib. Identities are the deterministic test
identities of the Rust test doubles: the Ed25519 seed is one byte repeated 32 times
(A=1 sender, B=2 provider, C=3 recipient). The identity ID is SHA-256(0x01 0x01 || public key).
Run from this directory: python3 generate.py
"""
import hashlib
import json
import nacl

from nacl.signing import SigningKey

SIGNING_PREFIX = b"CR2SE-MESSAGE-V1"
REQUEST_PREFIX = b"CR2SE-MESSAGE-RECOVERY-REQUEST-V1"
RECEIPT_PREFIX = b"CR2SE-MESSAGE-RECOVERY-RECEIPT-V1"


def identity(seed):
    key = SigningKey(bytes([seed]) * 32)
    public = bytes(key.verify_key)
    return key, public, hashlib.sha256(b"\x01\x01" + public).digest()


def u64(n):
    return n.to_bytes(8, "big")


a_key, a_pub, a_id = identity(1)
b_key, b_pub, b_id = identity(2)
c_key, c_pub, c_id = identity(3)

message_id = bytes(range(32))
payload = b"abc"
payload_hash = hashlib.sha256(payload).digest()
created_at = 1000
signing_bytes = (SIGNING_PREFIX + message_id + a_id + c_id + u64(created_at) + b"\x00"
                 + u64(len(payload)) + payload_hash + payload)
sender_signature = a_key.sign(signing_bytes).signature

placement_id = bytes([0x50]) * 32
recovery_id = bytes([0x60]) * 32
request_bytes = REQUEST_PREFIX + b_id + placement_id + message_id + payload_hash + recovery_id
recipient_signature = c_key.sign(request_bytes).signature
recovered_at = 1005
receipt_bytes = (RECEIPT_PREFIX + b_id + placement_id + message_id + a_id + c_id + payload_hash
                 + recovery_id + u64(recovered_at))
provider_signature = b_key.sign(receipt_bytes).signature

h = lambda b: b.hex()
vectors = {
    "source": "https://github.com/beothorn/CR2SE/blob/73edf2dc230bccb0f27f2ea6e06e938fff738862/modules/messaging/README.md",
    "generator": f"PyNaCl {nacl.__version__}, Python hashlib",
    "identities": {
        "sender": {"seed": 1, "publicKey": h(a_pub), "id": h(a_id)},
        "provider": {"seed": 2, "publicKey": h(b_pub), "id": h(b_id)},
        "recipient": {"seed": 3, "publicKey": h(c_pub), "id": h(c_id)},
    },
    "message": {
        "messageId": h(message_id), "senderCreatedAt": created_at, "encodingCode": 0,
        "payload": h(payload), "payloadHash": h(payload_hash),
        "signingBytes": h(signing_bytes), "senderSignature": h(sender_signature),
    },
    "recovery": {
        "placementId": h(placement_id), "recoveryId": h(recovery_id),
        "requestBytes": h(request_bytes), "recipientSignature": h(recipient_signature),
        "recoveredAt": recovered_at, "receiptBytes": h(receipt_bytes),
        "providerSignature": h(provider_signature),
    },
}
with open("vectors.json", "w") as out:
    json.dump(vectors, out, indent=2)
    out.write("\n")
