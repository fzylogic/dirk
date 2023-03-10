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
	cargo build $(release) --bin dirk

test:
	cargo test --all-features
	cargo fmt -- --check
	cargo clippy -- -D warnings

clean:
	cargo clean
	rm -rf usr

install_client:
	mkdir -p usr/local/bin
	cp target/$(target)/dirk usr/local/bin/

install_server:
	mkdir -p usr/local/bin
	cp target/$(target)/dirk-api usr/local/bin/

all: build install
 
help:
	@echo "usage: make [server|client] [debug=1]"
