ENVIRONMENT := debug

all: build

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

test: schema simulators
	cargo test -p tests
	scripts/run_sim_tests.sh $(ENVIRONMENT)

coverage: test
	zip -0 build/$(ENVIRONMENT)/ccov.zip `find . \( -name "asset_order_lockscript_sim*.gc*" \) -print`
	grcov build/$(ENVIRONMENT)/ccov.zip -s . -t lcov --llvm --branch --ignore-not-existing --ignore "/*" -o build/$(ENVIRONMENT)/lcov.info
	genhtml -o build/$(ENVIRONMENT)/coverage/ --rc lcov_branch_coverage=1 --show-details --highlight --ignore-errors source --legend build/$(ENVIRONMENT)/lcov.info

clean:
	cargo clean
	rm -rf build/$(ENVIRONMENT)

ci: all fmt build test simulators
