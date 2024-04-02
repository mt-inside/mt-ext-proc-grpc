set dotenv-load

default:
	@just --list --unsorted --color=always

NAME := "mt-ext-authz-grpc"
DH_USER := "mtinside"
GH_USER := "mt-inside"
DH_REPO := "docker.io/" + DH_USER + "/" + NAME
GH_REPO := "ghcr.io/" + GH_USER + "/" + NAME
TAG := `git describe --tags --always --abbrev`
TAGD := `git describe --tags --always --abbrev --dirty --broken`
CGR_ARCHS := "aarch64"#,amd64"
MELANGE := "melange"
APKO    := "apko"

tools-install-build-ci:
	rustup target add x86_64-unknown-linux-gnu
	rustup target add aarch64-unknown-linux-gnu
	rustup target add armv7-unknown-linux-gnueabihf
	rustup target add x86_64-apple-darwin
	rustup target add aarch64-apple-darwin

# non-blocking checks and advisories
analyze:
	cargo deny check

# Updates deps within their semver ranges
update-lock-file:
	cargo update

lint:
	cargo check
	cargo clippy -- -D warnings
	cargo doc # will fail if there's link issues etc
	cargo +nightly udeps --all-targets
	# ideally wouldn't use --direct
	cargo minimal-versions check --direct --workspace # check it builds with the min versions of all the deps (from their semver ranges)

test: lint
	cargo test --all

test-with-coverage: lint
	#!/bin/bash
	export RUSTFLAGS="-Cinstrument-coverage"
	export LLVM_PROFILE_FILE="{{NAME}}-%p-%m.profraw"
	cargo test --all
	# Convert the profraw files into lcov
	mkdir -p target/debug/coverage
	grcov . -s . --binary-path target/debug/ -t lcov --branch --ignore-not-existing --keep-only 'src/*' -o target/debug/coverage/
	rm -f *profraw

coverage-view: test-with-coverage
	mkdir -p target/debug/coverage
	grcov . -s . --binary-path target/debug/ -t html --branch --ignore-not-existing --keep-only 'src/*' -o target/debug/coverage/
	open target/debug/coverage/html/index.html

fmt:
	cargo +nightly fmt --all

build: test
	cargo build

build-ci *ARGS:
	cargo build --release {{ARGS}}

package: #test
	rm -rf ./packages/
	rm -rf ./target/
	{{MELANGE}} bump melange.yaml {{TAGD}}
	{{MELANGE}} keygen
	{{MELANGE}} build --arch {{CGR_ARCHS}} --signing-key melange.rsa melange.yaml

run *ARGS: test
	cargo run -- {{ARGS}}

image-local:
	{{APKO}} build --keyring-append melange.rsa.pub --arch {{CGR_ARCHS}} apko.yaml {{GH_REPO}}:{{TAG}} {{NAME}}.tar
	docker load < {{NAME}}.tar
image-publish:
	{{APKO}} login docker.io -u {{DH_USER}} --password "${DH_TOKEN}"
	{{APKO}} login ghcr.io   -u {{GH_USER}} --password "${GH_TOKEN}"
	{{APKO}} publish --keyring-append melange.rsa.pub --arch {{CGR_ARCHS}} apko.yaml {{GH_REPO}}:{{TAG}} {{DH_REPO}}:{{TAG}}
