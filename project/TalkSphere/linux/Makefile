CC=gcc
CFLAGS=-Wall -Wextra -Wpedantic -std=c11 -pthread
SRC=src/main.c
BIN=talksphere

all: $(BIN)

$(BIN): $(SRC)
	$(CC) $(CFLAGS) $(SRC) -o $(BIN)

clean:
	rm -f $(BIN)

.PHONY: all clean
