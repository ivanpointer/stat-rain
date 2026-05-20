.PHONY: build test bench profile run run-fake-idle run-fake-hot stress-cpu fmt check clean

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

run-fake-idle:
	$(CARGO) run -- run --simulate-metric cpu=0.05 --simulate-metric memory=0.25 --simulate-metric thermal_zone=42:0.2

run-fake-hot:
	$(CARGO) run -- run --simulate-metric cpu=1.0 --simulate-metric memory=0.9 --simulate-metric thermal_zone=95:0.95

stress-cpu:
	$(CARGO) run -- stress-cpu

fmt:
	$(CARGO) fmt --all

check:
	$(CARGO) check --all-targets

clean:
	$(CARGO) clean
