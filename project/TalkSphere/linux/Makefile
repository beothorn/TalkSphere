CC=gcc
CFLAGS=-Wall -Wextra -Wpedantic -std=c11 -pthread -Isrc
SRC=$(shell find src -name "*.c")
BUILD_DIRECTORY=build
BIN=$(BUILD_DIRECTORY)/talksphere

all: $(BIN)

$(BIN): $(SRC) | $(BUILD_DIRECTORY)
	$(CC) $(CFLAGS) $(SRC) -o $(BIN)

$(BUILD_DIRECTORY):
	mkdir -p $(BUILD_DIRECTORY)

clean:
	rm -rf $(BUILD_DIRECTORY)

test: all
	bash ./test/run_tests.sh

.PHONY: all clean test
