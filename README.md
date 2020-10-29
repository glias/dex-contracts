# dex-contracts

The contracts of DEX on Nervos CKB using [Capsule](https://github.com/nervosnetwork/capsule)

### Pre-requirement

- [capsule](https://github.com/nervosnetwork/capsule) >= 0.4.1
- [ckb-cli](https://github.com/nervosnetwork/ckb-cli) >= 0.35.0
- [secp256k1_blake2b_sighash_all_dual](https://github.com/nervosnetwork/ckb-miscellaneous-scripts/blob/master/c/secp256k1_blake2b_sighash_all_dual.c) which supports loaded as a shared library.

> Note: Capsule uses docker to build contracts and run tests. https://docs.docker.com/get-docker/
> and docker and ckb-cli must be accessible in the PATH in order for them to be used by Capsule.

### Getting Started

- Init submodules:

```
git submodule init && git submodule update -r --init
```

- Build the shared binary `secp256k1_blake2b_sighash_all_dual`:

```
cd ckb-miscellaneous-scripts && git submodule init && git submodule update

make install-tools && make all-via-docker
```

- Build contracts:

```sh
capsule build
```

- Run tests

```sh
capsule test
```

### Deployment

#### 1. Update the deployment configurations

Open `deployment.toml` :

- cells describes which cells to be deployed.

  - `name`: Define the reference name used in the deployment configuration.
  - `enable_type_id` : If it is set to true means create a type_id for the cell.
  - `location` : Define the script binary path.
  - `dep_groups` describes which dep_groups to be created. Dep Group is a cell which bundles several cells as its members. When a dep group cell is used in cell_deps, it has the same effect as adding all its members into cell_deps. In our case, we don’t need dep_groups.

- `lock` describes the lock field of the new deployed cells.It is recommended to set lock to the address(an address that you can unlock) of deployer in the dev chain and in the testnet, which is easier to update the script.

#### 2. Build release version of the script

The release version of script doesn’t include debug symbols which makes the size smaller.

```sh
capsule build --release
```

#### 3. Deploy the script

```sh
capsule deploy --address <ckt1....> --fee 0.001
```

If the `ckb-cli` has been installed and `dev-chain` RPC is connectable, you will see the deployment plan:

new_occupied_capacity and total_occupied_capacity refer how much CKB to store cells and data.
txs_fee_capacity refers how much CKB to pay the transaction fee.

```
Deployment plan:
---
migrated_capacity: 0.0 (CKB)
new_occupied_capacity: 125256.0 (CKB)
txs_fee_capacity: 0.03 (CKB)
total_occupied_capacity: 125256.0 (CKB)
recipe:
  cells:
    - name: order-book-contract
      index: 0
      tx_hash: "0xcdfd397823f6a130294c72fbe397c469d459b83db401296c291db7b170b15839"
      occupied_capacity: 33838.0 (CKB)
      data_hash: "0x8ee3aaeaa0d7eaecee8e676b6d53eff3ee38d0256c58038eee7b0baaefcdcf8c"
      type_id: "0x9c833b9ebd4259ca044d2c47c5e51b7fc25380b07291e54b248d3808f08ed7fd"
  dep_groups: []
```

#### 4. Type yes or y and input the password to unlock the account.

```
send cell_tx 0xcdfd397823f6a130294c72fbe397c469d459b83db401296c291db7b170b15839
Deployment complete
```

Now the dex script has been deployed, you can refer to this script by using `tx_hash: 0xcdfd397823f6a130294c72fbe397c469d459b83db401296c291db7b170b15839 index: 0` as `out_point`(your tx_hash should be another value).
