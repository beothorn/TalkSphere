# Application logging

initialize installs the shared tracing subscriber once at application composition.
Operational events go to stderr, without ANSI escape codes or payload formatting;
stdout remains reserved for intentional CLI results. RUST_LOG defaults to info and
supports level/target filters such as warn,talksphere::modules::transport=debug.
Regex matching against field values is disabled. Malformed/non-Unicode filters or
subscriber installation failure return fixed diagnostics and prevent runtime startup.
No protocol module installs or owns this subscriber. Help/version skip initialization.
Use TRACE only for diagnosis; application code must still exclude keys and payloads.
