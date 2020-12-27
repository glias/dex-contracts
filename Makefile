ENVIRONMENT := debug

simulators:
	CARGO_INCREMENTAL=0 RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Copt-level=0 -Clink-dead-code -Coverflow-checks=off -Zpanic_abort_tests -Cpanic=abort" RUSTDOCFLAGS="-Cpanic=abort" cargo build -p natives --target-dir=target
	mkdir -p build/$(ENVIRONMENT)
	cp target/$(ENVIRONMENT)/asset-order-lockscript-sim build/$(ENVIRONMENT)/asset-order-lockscript-sim

schema:
	make -C tests schema

fmt:
	cd contracts/asset-order-lockscript && cargo fmt --all
	cd tests && cargo fmt --all

build:
	capsule build

deps:
	cd deps/ckb-dyn-lock && make all-via-docker

test: simulators
	cargo test -p tests
	scripts/run_sim_tests.sh $(ENVIRONMENT)

ci: fmt build test simulators
