# CLI entry point

`run(arguments)` takes operating-system arguments excluding the executable name,
validates UTF-8 and dispatches documented commands. It prints help/version on
stdout, usage/runtime failures on stderr, and returns exit status 0 for help or
version, 2 for invalid input, and 1 for unavailable server integration. It does
not install protocol logging or manufacture credentials.

Explicit identity-init and identity-show commands require --identity-store DIRECTORY.
They return only the public canonical ID; initialization never overwrites keys and
inspection never generates them. The Linux protected store enforces ownership and
permissions; unsupported platforms fail explicitly. Detailed command/option help
is available through help identity-init, help identity-show and help --identity-store.
