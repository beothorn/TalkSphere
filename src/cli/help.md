# Shared help definitions

The internal topic table drives both overview and detailed command/option help.
Aliases include option spellings. Rendering explains usage, current server
availability and exit status; adding a command/option requires updating the
table, parser and CLI integration tests together.

Overview and serve help report completed Transport/Cryptography modules and link
the Cryptography plan and main roadmap for the Identity handoff. The serve command
continues to explain missing Identity/server composition and exits with status 1.
