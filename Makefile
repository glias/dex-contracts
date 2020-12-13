schema:
	make -C tests schema

fmt:
	cd contracts/asset-order-lockscript && cargo fmt --all
	cd tests && cargo fmt --all

build:
	capsule build

test:
	capsule test

ci: fmt build test
