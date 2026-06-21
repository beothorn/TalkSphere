# Application module

This module owns lightweight command orchestration for the Linux command-line runtime.

It is responsible for parsing startup input, resolving the app storage path, ensuring app files exist for commands that need local state, and delegating each command to its owning module application.

`main.c` stays intentionally small so the process entrypoint does not need to understand encryption, ledger, offerings, credit withdraw, storage, or network details.
