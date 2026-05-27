CC=gcc
CFLAGS=-Wall -Wextra -Wpedantic -std=c11 -pthread -Isrc
SRC=$(shell find src -name "*.c")
BIN=talksphere

all: $(BIN)

$(BIN): $(SRC)
	$(CC) $(CFLAGS) $(SRC) -o $(BIN)

clean:
	rm -f $(BIN)

test: all
	./tests/run_tests.sh

.PHONY: all clean test
