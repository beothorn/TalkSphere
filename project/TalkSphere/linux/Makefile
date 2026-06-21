CC=gcc
COMMON_CFLAGS=-Wall -Wextra -Wpedantic -std=c11 -pthread -Isrc
RELEASE_CFLAGS=-O2
DEBUG_CFLAGS=-g3 -O0
LDLIBS=-ldl
SRC=$(shell find src -name "*.c")
BUILD_DIRECTORY=build
DEBUG_BUILD_DIRECTORY=$(BUILD_DIRECTORY)/debug
BIN=$(BUILD_DIRECTORY)/talksphere
DEBUG_BIN=$(DEBUG_BUILD_DIRECTORY)/talksphere

all: $(BIN)

$(BIN): $(SRC) | $(BUILD_DIRECTORY)
	$(CC) $(COMMON_CFLAGS) $(RELEASE_CFLAGS) $(SRC) -o $(BIN) $(LDLIBS)

debug: $(DEBUG_BIN)

$(DEBUG_BIN): $(SRC) | $(DEBUG_BUILD_DIRECTORY)
	$(CC) $(COMMON_CFLAGS) $(DEBUG_CFLAGS) $(SRC) -o $(DEBUG_BIN) $(LDLIBS)

$(BUILD_DIRECTORY):
	mkdir -p $(BUILD_DIRECTORY)

$(DEBUG_BUILD_DIRECTORY):
	mkdir -p $(DEBUG_BUILD_DIRECTORY)

clean:
	rm -rf $(BUILD_DIRECTORY)

test: all
	bash ./test/run_tests.sh

.PHONY: all clean debug test
