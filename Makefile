.PHONY: build test bench profile run run-socket run-fake-idle run-fake-hot send-cpu-low send-cpu-mid send-cpu-hot send-message send-info send-success send-warning send-error send-stale send-metric-error clear-status stress-cpu fmt check clean

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

run-socket:
	$(CARGO) run -- run --socket /tmp/stat-rain.sock

run-fake-idle:
	$(CARGO) run -- run --simulate-metric cpu=0.05 --simulate-metric memory=0.25 --simulate-metric thermal_zone=42:0.2

run-fake-hot:
	$(CARGO) run -- run --simulate-metric cpu=1.0 --simulate-metric memory=0.9 --simulate-metric thermal_zone=95:0.95

send-cpu-low:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --metric cpu --value 0.02

send-cpu-mid:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --metric cpu --value 0.50

send-cpu-hot:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --metric cpu --value 0.99

send-message:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --message "$(MSG)" --class "$(or $(CLASS),info)" $(if $(TTL_MS),--ttl-ms $(TTL_MS),)

send-info:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --message "$(MSG)" --class info

send-success:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --message "$(MSG)" --class success

send-warning:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --message "$(MSG)" --class warning

send-error:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --message "$(MSG)" --class error

send-stale:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --metric "$(METRIC)" --stale $(if $(REASON),--reason "$(REASON)",)

send-metric-error:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --metric "$(METRIC)" --error $(if $(REASON),--reason "$(REASON)",)

clear-status:
	$(CARGO) run -- send --socket /tmp/stat-rain.sock --metric "$(METRIC)" --clear-status

stress-cpu:
	$(CARGO) run -- stress-cpu

fmt:
	$(CARGO) fmt --all

check:
	$(CARGO) check --all-targets

clean:
	$(CARGO) clean
