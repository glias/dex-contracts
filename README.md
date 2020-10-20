# dex-contracts

The contracts of DEX on Nervos CKB using [Capsule](https://github.com/nervosnetwork/capsule)

### Pre-requirement

- capsule > 0.3.0
- [secp256k1_blake2b_sighash_all_dual](https://github.com/nervosnetwork/ckb-miscellaneous-scripts/blob/master/c/secp256k1_blake2b_sighash_all_dual.c) which supports loaded as a shared library.

### Getting Started

Init submodules:

```
git submodule init && git submodule update -r --init
```

Build the shared binary `secp256k1_blake2b_sighash_all_dual`:

```
cd ckb-miscellaneous-scripts && git submodule init && git submodule update

make install-tools && make all-via-docker
```

Build contracts:

```sh
capsule build
```

Run tests:

```sh
capsule test
```
