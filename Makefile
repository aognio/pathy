.PHONY: build check test clean run clippy fmt fmt-check tiny

NAME := pathy
TARGET := target/release/$(NAME)

build:
	cargo build --release

tiny:
	CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
	CARGO_PROFILE_RELEASE_LTO=true \
	CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
	CARGO_PROFILE_RELEASE_PANIC=abort \
	CARGO_PROFILE_RELEASE_STRIP=symbols \
	cargo build --release

check:
	cargo check

test:
	cargo test

clean:
	cargo clean

run:
	cargo run --release

clippy:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt

fmt-check:
	cargo fmt -- --check
