schema:
	make -C tests schema

fmt:
	cd contracts/order-book-contract && cargo fmt --all
	cd tests && cargo fmt --all

build:
	capsule build

test:
	capsule test

ci: fmt build test
