use super::*;
use ckb_dyn_lock::test_tool;

const ERR_CANCEL_ORDER_WITHOUT_WITNESS: i8 = 25;
const ERR_USER_LOCK_NOT_FOUND: i8 = 26;
const ERR_USER_LOCK_SCRIPT_ENCODING: i8 = 27;
const ERR_USER_LOCK_HASH_NOT_MATCH: i8 = 28;
const ERR_UNKNOWN_USER_LOCK_HASH_TYPE: i8 = 29;
const ERR_USER_LOCK_CELL_DEP_NOT_FOUND: i8 = 30;
const ERR_DYNAMIC_LOADING_MEMORY_NOT_ENOUGH: i8 = 34;

// secp256k1_blake160_sighash_all lock error code
const ERR_SECP256K1_WRONG_KEY: i8 = -31;

#[test]
fn test_directly_cancel_order_using_signature_witness() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = DynLock::eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    let (secp256k1_keccak256_out_point, keccak256_deps) = DynLock::deploy(&mut context);
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
            cell_deps: Some(keccak256_deps),
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
fn test_directly_cancel_order_using_witness_redirect_to_forked_dyn_lock() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = DynLock::eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    let (_secp256k1_keccak256_dyn_out_point, keccak256_deps) = DynLock::deploy(&mut context);
    let secp256k1_keccak256_bin = binary::get(Binary::Secp256k1Keccak256SighashAll);
    let secp256k1_keccak256_out_point =
        context.deploy_cell(secp256k1_keccak256_bin.to_vec().into());

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
            cell_deps: Some(keccak256_deps),
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
    let pubkey_hash = Secp256k1Lock::blake160(&pubkey.serialize()).to_vec();

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
fn test_err_cancel_order_use_secp256k1_lockscript_with_wrong_key() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = Secp256k1Lock::blake160(&pubkey.serialize()).to_vec();
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

    // Error: sign tx use wrong key
    let tx = Secp256k1Lock::sign_tx(tx, &wrong_privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_SECP256K1_WRONG_KEY, 0));
}

#[test]
fn test_err_cancle_order_without_witness() {
    let mut context = Context::default();

    // Deploy always success script as user lock
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    let always_success_dep = CellDep::new_builder()
        .out_point(always_success_out_point.clone())
        .build();
    let always_success_lock_script = context
        .build_script(&always_success_out_point, Default::default())
        .expect("always success lock script");

    // Error: no witness
    let cancel_input = OrderInput::AnyUnlock {
        cell_deps: Some(vec![always_success_dep]),
        cell:      FreeCell::new(100_00_000_000),
        lock:      always_success_lock_script.clone(),
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
            custom_lock_args: Some(always_success_lock_script.calc_script_hash().as_bytes()),
            witness: None,
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![order_input, cancel_input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_CANCEL_ORDER_WITHOUT_WITNESS, 0));
}

#[test]
fn test_err_directly_cancel_user_lock_not_found() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = DynLock::eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    let (secp256k1_keccak256_out_point, keccak256_deps) = DynLock::deploy(&mut context);
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

        // Error: no user lock
        let witness = WitnessArgs::new_builder().build();

        OrderInput::Order {
            cell_deps: Some(keccak256_deps),
            cell,
            custom_lock_args: Some(keccak256_lock_script.calc_script_hash().as_bytes()),
            witness: Some(witness.as_bytes()),
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = test_tool::secp256k1_keccak256::sign_tx(tx, &privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_USER_LOCK_NOT_FOUND, 0));
}

#[test]
fn test_err_directly_cancel_user_lock_script_encoding() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = DynLock::eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    let (secp256k1_keccak256_out_point, keccak256_deps) = DynLock::deploy(&mut context);
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

        // Error: pass empty user lock bytes
        let witness = WitnessArgs::new_builder()
            .input_type(Some(Bytes::new()).pack())
            .build();

        OrderInput::Order {
            cell_deps: Some(keccak256_deps),
            cell,
            custom_lock_args: Some(keccak256_lock_script.calc_script_hash().as_bytes()),
            witness: Some(witness.as_bytes()),
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = test_tool::secp256k1_keccak256::sign_tx(tx, &privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_USER_LOCK_SCRIPT_ENCODING, 0));
}

#[test]
fn test_err_directly_cancel_order_user_lock_hash_not_match() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = DynLock::eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    let (secp256k1_keccak256_out_point, keccak256_deps) = DynLock::deploy(&mut context);
    let keccak256_lock_script = context
        .build_script(&secp256k1_keccak256_out_point, eth_pubkey)
        .expect("build secp256k1 keccak256 lock script");

    // Error: pass another lock script
    let another_lock_script = context
        .build_script(&secp256k1_keccak256_out_point, Default::default())
        .expect("build another lock script");

    let order_input = {
        let cell = OrderCell::builder()
            .capacity_dec(1000, 8)
            .sudt_amount(0)
            .order_amount_dec(50, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build();

        let witness = WitnessArgs::new_builder()
            .input_type(Some(another_lock_script.as_bytes()).pack())
            .build();

        OrderInput::Order {
            cell_deps: Some(keccak256_deps),
            cell,
            custom_lock_args: Some(keccak256_lock_script.calc_script_hash().as_bytes()),
            witness: Some(witness.as_bytes()),
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = test_tool::secp256k1_keccak256::sign_tx(tx, &privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_USER_LOCK_HASH_NOT_MATCH, 0));
}

#[test]
fn test_err_directly_cancel_order_unknown_user_lock_hash_type() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = DynLock::eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    let (secp256k1_keccak256_out_point, keccak256_deps) = DynLock::deploy(&mut context);
    let keccak256_lock_script = context
        .build_script(&secp256k1_keccak256_out_point, eth_pubkey)
        .expect("build secp256k1 keccak256 lock script");

    // Error: wrong hash type
    let wrong_hash_type_lcok_script = keccak256_lock_script
        .as_builder()
        .hash_type(2.into())
        .build();

    let order_input = {
        let cell = OrderCell::builder()
            .capacity_dec(1000, 8)
            .sudt_amount(0)
            .order_amount_dec(50, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build();

        let witness = WitnessArgs::new_builder()
            .input_type(Some(wrong_hash_type_lcok_script.as_bytes()).pack())
            .build();

        OrderInput::Order {
            cell_deps: Some(keccak256_deps),
            cell,
            custom_lock_args: Some(wrong_hash_type_lcok_script.calc_script_hash().as_bytes()),
            witness: Some(witness.as_bytes()),
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = test_tool::secp256k1_keccak256::sign_tx(tx, &privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_UNKNOWN_USER_LOCK_HASH_TYPE, 0));
}

#[test]
fn test_err_directly_cancel_user_lock_cell_dep_not_found() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = DynLock::eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    let (secp256k1_keccak256_out_point, _keccak256_deps) = DynLock::deploy(&mut context);
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

        // Error: no cell deps
        OrderInput::Order {
            cell_deps: Some(vec![]),
            cell,
            custom_lock_args: Some(keccak256_lock_script.calc_script_hash().as_bytes()),
            witness: Some(witness.as_bytes()),
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = test_tool::secp256k1_keccak256::sign_tx(tx, &privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_USER_LOCK_CELL_DEP_NOT_FOUND, 0));
}

#[test]
fn test_err_directly_cancel_order_using_witness_dynamic_loding() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let eth_pubkey = DynLock::eth_pubkey(pubkey);

    let mut context = Context::default();

    // Deploy dependencies
    //  Error: use lock script which doesn't support dynamic loading
    let (_secp256k1_keccak256_dyn_out_point, mut keccak256_deps) = DynLock::deploy(&mut context);
    let secp256k1_keccak256_bin = binary::get(Binary::Secp256k1Keccak256SighashAll);
    let secp256k1_keccak256_out_point =
        context.deploy_cell(secp256k1_keccak256_bin.to_vec().into());
    let secp256k1_keccak256_dep = CellDep::new_builder()
        .out_point(secp256k1_keccak256_out_point.clone())
        .build();
    keccak256_deps.push(secp256k1_keccak256_dep);

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
            cell_deps: Some(keccak256_deps),
            cell,
            custom_lock_args: Some(keccak256_lock_script.calc_script_hash().as_bytes()),
            witness: Some(witness.as_bytes()),
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = test_tool::secp256k1_keccak256::sign_tx(tx, &privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_DYNAMIC_LOADING_MEMORY_NOT_ENOUGH, 0));
}
