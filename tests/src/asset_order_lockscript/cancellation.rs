use super::*;

// secp256k1_blake160_sighash_all lock error code
const ERR_SECP256K1_WRONG_KEY: i8 = -31;

#[test]
fn test_directly_cancel_order_using_signature_witness() {
    use ckb_dyn_lock::locks::binary::{self, Binary};
    use ckb_dyn_lock::test_tool;

    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    let secp256k1_data_bin = binary::get(Binary::Secp256k1Data);
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();

    let secp256k1_keccak256_bin = binary::get(Binary::Secp256k1Keccak256SighashAllDual);
    let secp256k1_keccak256_out_point =
        context.deploy_cell(secp256k1_keccak256_bin.to_vec().into());
    let secp256k1_keccak256_dep = CellDep::new_builder()
        .out_point(secp256k1_keccak256_out_point.clone())
        .build();

    let keccak256_lock_script = context
        .build_script(&secp256k1_keccak256_out_point, eth_pubkey)
        .expect("build secp256k1 keccak256 lock script");

    let order_input = {
        let cell = OrderCell::builder()
            .capacity_dec(1000, 8)
            .sudt_amount(0)
            .order_amount_dec(50, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build();

        let witness = WitnessArgs::new_builder()
            .input_type(Some(keccak256_lock_script.as_bytes()).pack())
            .build();

        OrderInput::Order {
            cell_deps: Some(vec![secp256k1_data_dep, secp256k1_keccak256_dep]),
            cell,
            custom_lock_args: Some(keccak256_lock_script.calc_script_hash().as_bytes()),
            witness: Some(witness.as_bytes()),
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = test_tool::secp256k1_keccak256::sign_tx(tx, &privkey);
    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_cancel_order_use_secp256k1_lockscript() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = blake160(&pubkey.serialize()).to_vec();

    let mut context = Context::default();
    let (secp256k1_lock_out_point, secp256k1_lock_deps) = Secp256k1Lock::deploy(&mut context);
    let secp256k1_lock_script = context
        .build_script(&secp256k1_lock_out_point, pubkey_hash.into())
        .expect("secp256k1 lock script");

    let cancel_input = OrderInput::AnyUnlock {
        cell_deps: Some(secp256k1_lock_deps),
        cell:      FreeCell::new(100_00_000_000),
        lock:      secp256k1_lock_script.clone(),
        witness:   Bytes::new(),
    };

    let order_input = {
        let cell = OrderCell::builder()
            .capacity_dec(1000, 8)
            .sudt_amount(0)
            .order_amount_dec(50, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build();

        OrderInput::Order {
            cell_deps: None,
            cell,
            custom_lock_args: Some(secp256k1_lock_script.calc_script_hash().as_bytes()),
            witness: None,
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![cancel_input, order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = Secp256k1Lock::sign_tx(tx, &privkey);
    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_cancel_order_use_secp256k1_lockscript_with_wrong_key() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = blake160(&pubkey.serialize()).to_vec();
    let wrong_privkey = Generator::random_privkey();

    let mut context = Context::default();
    let (secp256k1_lock_out_point, secp256k1_lock_deps) = Secp256k1Lock::deploy(&mut context);
    let secp256k1_lock_script = context
        .build_script(&secp256k1_lock_out_point, pubkey_hash.into())
        .expect("secp256k1 lock script");

    let cancel_input = OrderInput::AnyUnlock {
        cell_deps: Some(secp256k1_lock_deps),
        cell:      FreeCell::new(100_00_000_000),
        lock:      secp256k1_lock_script.clone(),
        witness:   Bytes::new(),
    };

    let order_input = {
        let cell = OrderCell::builder()
            .capacity_dec(1000, 8)
            .sudt_amount(0)
            .order_amount_dec(50, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build();

        OrderInput::Order {
            cell_deps: None,
            cell,
            custom_lock_args: Some(secp256k1_lock_script.calc_script_hash().as_bytes()),
            witness: None,
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![cancel_input, order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = Secp256k1Lock::sign_tx(tx, &wrong_privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_SECP256K1_WRONG_KEY, 0));
}
