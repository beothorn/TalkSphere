# TalkSphere (Linux Prototype)

This prototype runs one server instance per process and waits for inbound TCP messages.

## Build

```bash
make
```

## Run

```bash
./talksphere <instance_port> <reserved_port>
```

- `instance_port`: the TCP port where this TalkSphere instance listens.
- `reserved_port`: reserved for next iterations (today we only validate it and keep it different from `instance_port`).

## Current Messages

- `CONNECT:<target_host>:<target_port>,FROM:<reply_host>:<reply_port>`
  - When received, the instance connects to `<target_host>:<target_port>` and sends `MESSAGE:Hello`.
  - Then it connects to `<reply_host>:<reply_port>` and sends `MESSAGE:Hello`.
- `MESSAGE:<text>`
  - When received, the instance prints `<text>`.

## Step-by-step demo with two instances and nc

Terminal 1:
```bash
./talksphere 8734 8999
```

Terminal 2:
```bash
./talksphere 8735 9898
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

