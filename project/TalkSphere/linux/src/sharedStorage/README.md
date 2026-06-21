# Shared storage module

This module owns the Linux shared-storage file manager.

The integration with peers is intentionally not here yet. This code prepares the local storage side so another module can later call a small API to store, recover, delete, clean up, or query shared files.

## Responsibilities

- Store a byte array under the app storage directory or under `$HOME/.local/share/talksphere/sharedStorage` when no directory is provided.
- Keep SQLite metadata for each stored file in `sharedStorage/file_manager.sqlite`.
- Recover stored bytes by `shared_file_id` and `owner_id`.
- Force delete one entry by removing its file first and then removing its metadata.
- Clean up expired entries by deleting expired files and then deleting expired metadata.
- Run read-only SQL queries against the file manager metadata so future integration code can inspect the storage state.
- Own shared-storage command behavior in `shared_storage_application.c`.

## Public API

- `shared_storage_store_data` stores bytes and writes the metadata row.
- `shared_storage_recover_data` reads bytes back when the caller provides the same file id and owner id.
- `shared_storage_delete_entry` force deletes one stored entry.
- `shared_storage_clean_up_expired_entries` deletes every entry whose expiration is at or before the supplied current time.
- `shared_storage_query_file_manager` accepts read-only `SELECT`, `WITH`, and `PRAGMA` statements and calls a row callback for each result row.

The older placeholder functions are still present because earlier tasks and tests already referenced them:

- `shared_storage_share_available_storage`
- `shared_storage_recover_sold_storage`
- `shared_storage_clear_aged_storage`

## Storage layout

When the caller passes `/some/app/root`, shared storage uses:

```text
/some/app/root/sharedStorage/
  file_manager.sqlite
  files/
    <hex-owner-id>_<hex-shared-file-id>.bin
```

File names are hex-encoded from the owner id and file id. This keeps untrusted identifiers from becoming path traversal input.

## Boundaries

- `fileSystem` owns directories, file paths, byte writes, byte reads, and file deletion.
- `management` owns SQLite schema and metadata queries.
- `shared_storage.c` coordinates both parts. This is where the module decides operation order, such as "write file before saving metadata" and "delete file before deleting metadata".

The management module dynamically loads `libsqlite3.so.0`. This keeps the code buildable on systems that have the SQLite runtime but not the development header installed. If a target distribution does not ship the runtime library, install the SQLite runtime package before using this module.

## Metadata table

`shared_files` contains:

- `shared_file_id`
- `owner_id`
- `stored_file_path`
- `file_byte_count`
- `expiration_time_seconds`
- `created_time_seconds`

The primary key is `(shared_file_id, owner_id)`, so the same file id can exist for different owners and storing the same owner/file pair updates the metadata.

## Testing

The unit test in `test/sharedStorage/shared_storage_test.c` covers:

- storing and recovering bytes;
- rejecting recovery with the wrong owner id;
- querying metadata through SQL;
- rejecting mutating SQL through the query API;
- force deleting an entry;
- cleaning up expired entries while preserving active entries;
- invalid input failures.
