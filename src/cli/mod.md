# CLI entry point

`run(arguments)` takes operating-system arguments excluding the executable name,
validates UTF-8 and dispatches documented commands. It prints help/version on
stdout, usage/runtime failures on stderr, and returns exit status 0 for help or
version, 2 for invalid input, and 1 for unavailable server integration. It does
not install protocol logging or manufacture credentials.
