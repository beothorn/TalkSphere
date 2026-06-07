# Shared storage management

This folder isolates SQLite metadata work for shared storage.

It creates and queries the `shared_files` table, but it does not read or delete the stored bytes. That separation keeps database behavior testable without mixing it with file-system rules.

The module loads the SQLite runtime dynamically through `libsqlite3.so.0`. The purpose is to avoid exposing SQLite headers to the rest of the codebase and to keep this responsibility contained inside the management domain.
