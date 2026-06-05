# TalkSphere (Linux Prototype)

This prototype runs one server instance per process and waits for inbound TCP messages.

## Build

```bash
make
```

## Run

```bash
build/talksphere <listen_port> <peer_port> [home_folder]
```

- `listen_port`: the TCP port where this TalkSphere instance listens.
- `peer_port`: reserved for next iterations. Today the app validates it and keeps it different from `listen_port`.
- `home_folder`: optional storage folder for this instance. Use a different folder for each local instance so each one has its own `id`, `offerings`, and `ledger`.

When `home_folder` is omitted, TalkSphere uses the default Linux app storage folder under `$XDG_DATA_HOME/talksphere` or `$HOME/.local/share/talksphere`.

## Run Two Local Instances

Use different ports and different home folders:

Terminal 1:

```bash
mkdir -p /tmp/talksphere-demo/alice
build/talksphere 9101 9102 /tmp/talksphere-demo/alice
```

Terminal 2:

```bash
mkdir -p /tmp/talksphere-demo/bob
build/talksphere 9201 9202 /tmp/talksphere-demo/bob
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

You can check the separate IDs from another terminal:

```bash
build/talksphere --id /tmp/talksphere-demo/alice
build/talksphere --id /tmp/talksphere-demo/bob
```

## Current Messages

- `CONNECT:<target_host>:<target_port>,FROM:<reply_host>:<reply_port>`
  - When received, the instance connects to `<target_host>:<target_port>` and sends `MESSAGE:Hello`.
  - Then it connects to `<reply_host>:<reply_port>` and sends `MESSAGE:Hello`.
- `MESSAGE:<text>`
  - When received, the instance prints `<text>`.
- `LIST_OFFERINGS`
  - When received, the instance returns the current JSON string from its own `offerings` file.

## Step-by-step demo with two instances and nc

Terminal 1:
```bash
mkdir -p /tmp/talksphere-demo/instance1
build/talksphere 8734 8999 /tmp/talksphere-demo/instance1
```

Terminal 2:
```bash
mkdir -p /tmp/talksphere-demo/instance2
build/talksphere 8735 9898 /tmp/talksphere-demo/instance2
```

Terminal 3 (listen for the callback hello on port 8999):
```bash
nc -l 8999
```

Terminal 4 (send connect command to instance 1):
```bash
printf 'CONNECT:localhost:8735,FROM:localhost:8999' | nc -N localhost 8734
```

Expected result:
- Terminal 2 prints `Hello` (instance 2 received `MESSAGE:Hello`).
- Terminal 3 receives `MESSAGE:Hello` (callback path from instance 1).

To ask instance 2 for its current offerings:

```bash
printf 'LIST_OFFERINGS' | nc -N localhost 8735
```

# TalkSphere Linux tests

Tests live under `test/<module>/<file>_test.*`.

Use a C test when the module can be called directly, for example:

```bash
test/argumentParsing/program_arguments_test.c
```

Use a shell test when the behavior needs the built `talksphere` binary, multiple processes, or TCP sockets.

## Run all tests

```bash
make test
```

This builds `build/talksphere` and then runs every test listed in `test/run_tests.sh`.

## Run one test

Build the application first when the test uses `build/talksphere`:

```bash
make
```

Then pass one or more test files to the runner:

```bash
bash test/run_tests.sh argumentParsing/program_arguments_test.c
bash test/run_tests.sh test/offerings/offerings_test.c
bash test/run_tests.sh test/network/socket_channel_test.sh
```

The runner compiles C tests into `build/test`.

## Add a test

Create the test beside the module it covers, then add the path and source mapping to `test/run_tests.sh`.

Examples:

```text
test/argumentParsing/program_arguments_test.c
test/files/app_files_test.c
test/network/socket_channel_test.sh
```
