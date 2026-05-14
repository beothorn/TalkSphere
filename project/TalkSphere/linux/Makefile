CC=gcc
CFLAGS=-Wall -Wextra -Wpedantic -std=c11
SRC=src/main.c
BIN=socket_app

all: $(BIN)

$(BIN): $(SRC)
	$(CC) $(CFLAGS) $(SRC) -o $(BIN)

clean:
	rm -f $(BIN)

.PHONY: all clean
