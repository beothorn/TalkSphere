# Operations journal

Records how recent control operations ended: request id, operation, lease (hex) when one is
involved, status `ok`, `failed` or `unknown`, and the time. It keeps the latest 128 records in
`operations.json`, replaced atomically with mode 0600, and holds no object content, keys or
peer-supplied text. `unknown` means the remote effect may have committed; inspecting the journal
never retries anything. A malformed file is refused rather than discarded; a write failure is logged
and does not fail the operation.
