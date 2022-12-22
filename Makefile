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
	cargo build $(release) --bin dirk-file --bin dirk-scan

test:
	cargo test
	cargo clippy
	cargo fmt -- --check

clean:
	cargo clean
	rm -rf usr

install_client:
	mkdir -p usr/local/bin
	cp target/$(target)/dirk-scan usr/local/bin/
	cp target/$(target)/dirk-file usr/local/bin/

install_server:
	mkdir -p usr/local/bin
	cp target/$(target)/dirk-api usr/local/bin/

all: build install
 
help:
	@echo "usage: make [server|client] [debug=1]"
