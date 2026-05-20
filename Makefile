.PHONY: build test bench profile run fmt check clean

CARGO ?= cargo

build:
	$(CARGO) build

test:
	$(CARGO) test

bench:
	$(CARGO) bench

profile:
	hyperfine --warmup 3 '$(CARGO) run -- --help'

run:
	$(CARGO) run -- run

fmt:
	$(CARGO) fmt --all

check:
	$(CARGO) check --all-targets

clean:
	$(CARGO) clean
