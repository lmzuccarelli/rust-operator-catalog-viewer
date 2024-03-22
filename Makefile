.PHONY: all test build clean

all: clean test build

LEVEL ?= "info"
TEST ?= ""
DIFF ?= false
DOCKER_DESTINATION ?= "docker://127.0.0.1:5000/test"

build: 
	cargo build

build-release:
	cargo build --release

test: clean
	CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='cargo-test-%p-%m.profraw' cargo test  -- --nocapture

test-by-name: clean
	CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='cargo-test-%p-%m.profraw' cargo test $(TEST) -- --nocapture

cover:
	grcov . --binary-path ./target/debug/deps/ -s . -t html --branch --ignore-not-existing --ignore '../*' --ignore "/*" --ignore "src/main.rs" --ignore "src/api/*"  -o target/coverage/html
	cp target/coverage/html/html/badges/flat.svg assets/

run-d2m:
	cargo run -- --config imagesetconfig.yaml --diff-tar $(DIFF)  --loglevel $(LEVEL) --destination $(DOCKER_DESTINATION)

run-m2d:
	cargo run -- --config imagesetconfig.yaml --diff-tar $(DIFF)  --loglevel $(LEVEL) --destination file://

clean:
	rm -rf cargo-test*
