debug ?=

$(info debug is $(debug))

ifdef debug
  release :=
  target :=debug
else
  release :=--release
  target :=release
endif

server:
	cargo build $(release) --bin dirk-api

client:
	cargo build $(release) --bin dirk-submit

test:
	cargo test
	cargo clippy

clean:
	cargo clean
	rm -rf usr

install:
	mkdir -p usr/local/bin
	cp target/$(target)/[a-zA-Z_\-]+ usr/local/bin/

all: build install
 
help:
	@echo "usage: make [server|client] [debug=1]"
