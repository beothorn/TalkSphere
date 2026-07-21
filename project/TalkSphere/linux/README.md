# TalkSphere CLI

This is a CLI implementation of the talksphere

## Build

```bash
make clean all
```

The built binary is `build/talksphere`.

## Command Line

`--dry-run` must appear before the command. It prints what TalkSphere would do instead of running the command.
`--home` must appear before the command. It changes the folder where configurations are.

Main commands:

```bash
talksphere run <listen_port> <peer_port>  
talksphere start
talksphere config get home
talksphere files home
talksphere encryption [--help|help|h]
talksphere ledger [--help|help|h]
talksphere network [--help|help|h]
talksphere offerings [--help|help|h]
talksphere talk -p <client_port> offerings
talksphere talk -p <client_port> message "message"
talksphere share [--help|help|h]
```

The home folder defaults to `$XDG_DATA_HOME/talksphere` or `$HOME/.local/share/talksphere`.  

## Log level

You can set the environment variable `TALKSPHERE_LOG_LEVEL` with the log level.  
The levels available are:  
 - trace
 - debug
 - info
 - warn
 - error
 - fatal

Example:

`TALKSPHERE_LOG_LEVEL=trace ./talksphere`

## Existing Commands

Run a server:

```bash
talksphere run <listen_port> <peer_port>
```

- `listen_port`: the TCP port where this TalkSphere instance listens.
- `peer_port`: stored in runtime configuration for peer-oriented flows and kept different from `listen_port`.  

Create the home files without starting a server:

```bash
talksphere --home ~/dev/talksphere_sandbox/Alice start
```

This creates the home folder, local identifier, offerings file, config file, and ledger directory. When a new identifier is created, the command prints:

```text
A new identifier was created: <identifier>
```

Print the resolved home folder:

```bash
talksphere config get home
talksphere files home
```

Print local ledger totals:

```bash
talksphere ledger credit_summary
```

Print local offerings:

```bash
talksphere offerings get
```

Ask a running local instance to fetch offerings from its configured peer:

```bash
talksphere talk -p <client_port> offerings
```

This command connects to the local instance listening on `<client_port>`. That instance calls the peer port it was started with, requests `LIST_OFFERINGS`, returns the peer offerings to the CLI, and the CLI prints them.

Ask a running local instance to send a message to its configured peer:

```bash
talksphere talk -p <client_port> message "hello world"
```

This command connects to the local instance listening on `<client_port>`. That instance sends `MESSAGE:hello world` to the peer port it was started with.

Create placeholder encryption key files:

```bash
talksphere encryption create
talksphere encryption recreate
```

The encryption module is still a placeholder, so these files are currently empty. `create` fails when key files already exist. `recreate` fails when key files do not exist.

Call the placeholder encryption functions:

```bash
talksphere encryption encrypt_message "message"
talksphere encryption sign_message "message"
```

These commands currently print empty outputs until real encryption and signing are implemented.

## Placeholder Commands

These commands parse and return a clear placeholder message, but the feature body is not implemented yet:

```bash
talksphere network ping <ip:port>
talksphere offerings <ip:port>
talksphere offerings add <offering options>
talksphere offerings edit <offering options>
talksphere offerings remove <offering>
talksphere share local ls
talksphere share remote ls
```

See `tasks/04/todo.md` for the current missing-functionality list.

## Run Two Local Instances

Use different ports and different home folders:

Terminal 1:

```bash
build/talksphere --home /tmp/talksphere-demo/alice start
build/talksphere --home /tmp/talksphere-demo/alice run 9101 9102
```

Terminal 2:

```bash
build/talksphere --home /tmp/talksphere-demo/bob start
build/talksphere --home /tmp/talksphere-demo/bob run 9201 9202
```

Each instance creates and reads its own files:

```text
/tmp/talksphere-demo/alice/id
/tmp/talksphere-demo/alice/offerings
/tmp/talksphere-demo/alice/ledger

/tmp/talksphere-demo/bob/id
/tmp/talksphere-demo/bob/offerings
/tmp/talksphere-demo/bob/ledger
```

You can inspect the generated IDs directly:

```bash
cat /tmp/talksphere-demo/alice/id
cat /tmp/talksphere-demo/bob/id
```

## Current Socket Messages

- `CONNECT:<target_host>:<target_port>,FROM:<reply_host>:<reply_port>`
  - When received, the instance connects to `<target_host>:<target_port>` and sends `MESSAGE:Hello`.
  - Then it connects to `<reply_host>:<reply_port>` and sends `MESSAGE:Hello`.
- `MESSAGE:<text>`
  - When received, the instance prints `<text>`.
- `LIST_OFFERINGS`
  - When received, the instance returns the current JSON string from its own `offerings` file.
- `FETCH_CONNECTED_PEER_OFFERINGS`
  - When received by a local instance, it asks the configured peer port for `LIST_OFFERINGS` and returns that response.
- `SEND_CONNECTED_PEER_MESSAGE:<text>`
  - When received by a local instance, it sends `MESSAGE:<text>` to the configured peer port and returns `OK`.

## Step-by-step Demo With Two Instances

Terminal 1:

```bash
mkdir -p /tmp/talksphere-demo/instance1
build/talksphere --home /tmp/talksphere-demo/instance1 run 8899 9900
```

Terminal 2:

```bash
mkdir -p /tmp/talksphere-demo/instance2
build/talksphere --home /tmp/talksphere-demo/instance2 run 9900 8899
```

Terminal 3:

```bash
build/talksphere talk -p 8899 offerings
```

Expected result: the command prints the offerings file from instance 2.

To send a message from instance 1 to instance 2:

```bash
build/talksphere talk -p 8899 message "hello world"
```

Expected result: instance 2 receives `hello world`. Message delivery spends one credit from the receiving instance identifier, so the peer needs credit before a message can be accepted.

## Tests

Tests live under `test/<module>/<file>_test.*`.

Use a C test when the module can be called directly, for example:

```bash
test/argumentParsing/program_arguments_test.c
```

Use a shell test when the behavior needs the built `talksphere` binary, multiple processes, or TCP sockets.

Run all tests:

```bash
make test
```

This builds `build/talksphere` and then runs every test listed in `test/run_tests.sh`.

Run one test:

```bash
make
bash test/run_tests.sh argumentParsing/program_arguments_test.c
bash test/run_tests.sh test/offerings/offerings_test.c
bash test/run_tests.sh test/network/socket_channel_test.sh
```

The runner compiles C tests into `build/test`.

To add a test, create it beside the module it covers, then add the path and source mapping to `test/run_tests.sh`.
