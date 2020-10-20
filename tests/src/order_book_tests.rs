use super::*;
use ckb_system_scripts::BUNDLED_CELL;
use ckb_testtool::context::Context;
use ckb_tool::ckb_crypto::secp::{Generator, Privkey};
use ckb_tool::ckb_error::assert_error_eq;
use ckb_tool::ckb_hash::{blake2b_256, new_blake2b};
use ckb_tool::ckb_script::ScriptError;
use ckb_tool::ckb_types::{
    bytes::Bytes,
    core::{Capacity, TransactionBuilder, TransactionView},
    packed::{self, *},
    prelude::*,
    H256,
};
use rand::{thread_rng, Rng};
use std::fs;

const MAX_CYCLES: u64 = 1000_0000;

fn blake160(data: &[u8]) -> [u8; 20] {
    let mut buf = [0u8; 20];
    let hash = blake2b_256(data);
    buf.clone_from_slice(&hash[..20]);
    buf
}

fn sign_tx(tx: TransactionView, key: &Privkey) -> TransactionView {
    const SIGNATURE_SIZE: usize = 65;

    let witnesses_len = tx.witnesses().len();
    let tx_hash = tx.hash();
    let mut signed_witnesses: Vec<packed::Bytes> = Vec::new();
    let mut blake2b = new_blake2b();
    let mut message = [0u8; 32];
    blake2b.update(&tx_hash.raw_data());
    // digest the first witness
    let witness = WitnessArgs::default();
    let zero_lock: Bytes = {
        let mut buf = Vec::new();
        buf.resize(SIGNATURE_SIZE, 0);
        buf.into()
    };
    let witness_for_digest = witness
        .clone()
        .as_builder()
        .lock(Some(zero_lock).pack())
        .build();
    let witness_len = witness_for_digest.as_bytes().len() as u64;
    blake2b.update(&witness_len.to_le_bytes());
    blake2b.update(&witness_for_digest.as_bytes());
    (1..witnesses_len).for_each(|n| {
        let witness = tx.witnesses().get(n).unwrap();
        let witness_len = witness.raw_data().len() as u64;
        blake2b.update(&witness_len.to_le_bytes());
        blake2b.update(&witness.raw_data());
    });
    blake2b.finalize(&mut message);
    let message = H256::from(message);
    let sig = key.sign_recoverable(&message).expect("sign");
    signed_witnesses.push(
        witness
            .as_builder()
            .lock(Some(Bytes::from(sig.serialize())).pack())
            .build()
            .as_bytes()
            .pack(),
    );
    for i in 1..witnesses_len {
        signed_witnesses.push(tx.witnesses().get(i).unwrap());
    }
    tx.as_advanced_builder()
        .set_witnesses(signed_witnesses)
        .build()
}

fn build_test_context(
    inputs_token: Vec<u64>,
    outputs_token: Vec<u64>,
    inputs_data: Vec<Bytes>,
    outputs_data: Vec<Bytes>,
    input_args: Vec<Bytes>,
    output_args: Vec<Bytes>,
) -> (Context, TransactionView) {
    // deploy dex script
    let mut context = Context::default();
    let dex_bin: Bytes = Loader::default().load_binary("order-book-contract");
    let dex_out_point = context.deploy_cell(dex_bin);

    // prepare inputs
    let mut inputs = vec![];
    for index in 0..inputs_token.len() {
        let dex_script = context
            .build_script(&dex_out_point, input_args.get(index).unwrap().clone())
            .expect("script");
        let token = inputs_token.get(index).unwrap();
        let capacity = Capacity::shannons(*token);
        let input_out_point = context.create_cell(
            CellOutput::new_builder()
                .capacity(capacity.pack())
                .lock(dex_script.clone())
                .build(),
            inputs_data.get(index).unwrap().clone(),
        );
        let input = CellInput::new_builder()
            .previous_output(input_out_point)
            .build();
        inputs.push(input);
    }

    // prepare outputs
    let mut outputs = vec![];
    for index in 0..outputs_token.len() {
        let dex_script = context
            .build_script(&dex_out_point, output_args.get(index).unwrap().clone())
            .expect("script");
        let token = outputs_token.get(index).unwrap();
        let capacity = Capacity::shannons(*token);
        let output = CellOutput::new_builder()
            .capacity(capacity.pack())
            .lock(dex_script.clone())
            .build();
        outputs.push(output);
    }

    let dex_script_dep = CellDep::new_builder().out_point(dex_out_point).build();
    let mut witnesses = vec![];
    for _ in 0..inputs.len() {
        witnesses.push(Bytes::new())
    }

    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(dex_script_dep)
        .witnesses(witnesses.pack())
        .build();
    (context, tx)
}

#[test]
// Assume the sudt decimal is 8 and the price 5 sudt/ckb
fn test_ckb_sudt_partial_order() {
    // input1: sudt_amount(50sudt 0x12A05F200u128) + dealt_amount(50sudt 0x12A05F200u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input2: sudt_amount(500sudt 0xBA43B7400u128) + dealt_amount(100sudt 0x2540BE400u128) + undealt_amount(200sudt 0x4A817C800u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000F2052A01000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B000000000000000000000000E40B5402000000000000000000000000C817A804000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output1: sudt_amount(200sudt 0x4A817C800u128)
    // output2: sudt_amount(349.55sudt 0x8237AF8C0u128) + dealt_amount(250sudt 0x5D21DBA00u128) + undealt_amount(50sudt 0x12A05F200u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let outputs_data = vec![
        Bytes::from(hex::decode("00C817A8040000000000000000000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A2308000000000000000000000000BA1DD205000000000000000000000000F2052A01000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
    );

    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("cycles: {}", cycles);
}

#[test]
fn test_ckb_sudt_all_order1() {
    // input1: sudt_amount(50sudt 0x12A05F200u128) + dealt_amount(50sudt 0x12A05F200u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5.2*10^10 0xC1B710800u64) + buy(00)

    // input2: sudt_amount(500sudt 0xBA43B7400u128) + dealt_amount(100sudt 0x2540BE400u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000F2052A01000000000000000000000000D6117E0300000000000000000000000008711B0C00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B000000000000000000000000E40B5402000000000000000000000000D6117E03000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output1: sudt_amount(200sudt 0x4A817C800u128)
    // output2: sudt_amount(349.55sudt 0x8237AF8C0u128)
    let outputs_data = vec![
        Bytes::from(hex::decode("00C817A8040000000000000000000000").unwrap()),
        Bytes::from(hex::decode("C0F87A23080000000000000000000000").unwrap()),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
    );

    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("cycles: {}", cycles);
}

#[test]
fn test_ckb_sudt_all_order2() {
    // input1: sudt_amount(0sudt 0x0u128) + dealt_amount(0sudt 0x0u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input2: sudt_amount(500sudt 0xBA43B7400u128) + dealt_amount(0sudt 0x0u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("000000000000000000000000000000000000000000000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B00000000000000000000000000000000000000000000000000000000D6117E03000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output1: sudt_amount(150sudt 0x37E11D600u128)
    // output2: sudt_amount(349.55sudt 0x8237AF8C0u128)
    let outputs_data = vec![
        Bytes::from(hex::decode("00D6117E030000000000000000000000").unwrap()),
        Bytes::from(hex::decode("C0F87A23080000000000000000000000").unwrap()),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
    );

    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("cycles: {}", cycles);
}

#[test]
fn test_ckb_sudt_all_order_capacity_error() {
    // input1: sudt_amount(50sudt 0x12A05F200u128) + dealt_amount(50sudt 0x12A05F200u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input2: sudt_amount(500sudt 0xBA43B7400u128) + dealt_amount(100sudt 0x2540BE400u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000F2052A01000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B000000000000000000000000E40B5402000000000000000000000000D6117E03000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output1: sudt_amount(200sudt 0x4A817C800u128)
    // output2: sudt_amount(349.55sudt 0x8237AF8C0u128)
    let outputs_data = vec![
        Bytes::from(hex::decode("00C817A8040000000000000000000000").unwrap()),
        Bytes::from(hex::decode("C0F87A23080000000000000000000000").unwrap()),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 740 = 1540 not 1530 (output2 capacity amount is error)
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 153000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
    );

    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    let script_cell_index = 1;
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(15).input_lock_script(script_cell_index)
    );
}


#[test]
// Assume the sudt decimal is 8 and the price 5 sudt/ckb
fn test_ckb_sudt_order_type_error() {
    // input1: sudt_amount(50sudt 0x12A05F200u128) + dealt_amount(50sudt 0x12A05F200u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input2: sudt_amount(500sudt 0xBA43B7400u128) + dealt_amount(100sudt 0x2540BE400u128) + undealt_amount(200sudt 0x4A817C800u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000F2052A01000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B000000000000000000000000E40B5402000000000000000000000000C817A804000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output1: sudt_amount(200sudt 0x4A817C800u128)
    // output2: sudt_amount(349.55sudt 0x8237AF8C0u128) + dealt_amount(250sudt 0x5D21DBA00u128) + undealt_amount(50sudt 0x12A05F200u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)
    let outputs_data = vec![
        Bytes::from(hex::decode("00C817A8040000000000000000000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A2308000000000000000000000000BA1DD205000000000000000000000000F2052A01000000000000000000000000743BA40B00000000").unwrap(),
        ),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
    );

    let tx = context.complete_tx(tx);

    // run
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    let script_cell_index = 1;
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(13).input_lock_script(script_cell_index)
    );
}

#[test]
fn test_ckb_sudt_all_order_price_not_match() {
    // input1: sudt_amount(0sudt 0x0u128) + dealt_amount(0sudt 0x0u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input2: sudt_amount(500sudt 0xBA43B7400u128) + dealt_amount(0sudt 0x0u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(6*10^10 0xDF8475800u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("000000000000000000000000000000000000000000000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B00000000000000000000000000000000000000000000000000000000D6117E030000000000000000000000005847F80D00000001").unwrap(),
        ),
    ];

    // output1: sudt_amount(150sudt 0x37E11D600u128)
    // output2: sudt_amount(349.55sudt 0x8237AF8C0u128)
    let outputs_data = vec![
        Bytes::from(hex::decode("00D6117E030000000000000000000000").unwrap()),
        Bytes::from(hex::decode("C0F87A23080000000000000000000000").unwrap()),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
    );

    let tx = context.complete_tx(tx);

    // run
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    let script_cell_index = 1;
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(15).input_lock_script(script_cell_index)
    );
}

#[test]
fn test_ckb_sudt_all_order_cell_data_format_error() {
    // input1: sudt_amount(0sudt 0x0u128 (error: 126 bits)) + dealt_amount(0sudt 0x0u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input2: sudt_amount(500sudt 0xBA43B7400u128) + dealt_amount(0sudt 0x0u128) + undealt_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("000000000000000000000000000000000000000000000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B00000000000000000000000000000000000000000000000000000000D6117E03000000000000000000000000743BA40B0000000001").unwrap(),
        ),
    ];

    // output1: sudt_amount(150sudt 0x37E11D600u128)
    // output2: sudt_amount(349.55sudt 0x8237AF8C0u128)
    let outputs_data = vec![
        Bytes::from(hex::decode("00D6117E030000000000000000000000").unwrap()),
        Bytes::from(hex::decode("C0F87A23080000000000000000000000").unwrap()),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("7e7a30e75685e4d332f69220e925575dd9b84676").unwrap()),
        Bytes::from(hex::decode("a53ce751e2adb698ca10f8c1b8ebbee20d41a842").unwrap()),
    ];
    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
    );

    let tx = context.complete_tx(tx);

    // run
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    let script_cell_index = 1;
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(9).input_lock_script(script_cell_index)
    );
}

#[test]
fn test_signature_basic() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = blake160(&pubkey.serialize());

    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("order-book-contract");
    let out_point = context.deploy_cell(contract_bin);

    let secp256k1_bin: Bytes =
        fs::read("../ckb-miscellaneous-scripts/build/secp256k1_blake2b_sighash_all_dual")
            .expect("load secp256k1")
            .into();
    let secp256k1_out_point = context.deploy_cell(secp256k1_bin);
    let secp256k1_dep = CellDep::new_builder()
        .out_point(secp256k1_out_point)
        .build();

    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();

    // prepare scripts
    let lock_script = context
        .build_script(&out_point, pubkey_hash.to_vec().into())
        .expect("script");
    let lock_script_dep = CellDep::new_builder().out_point(out_point).build();

    // prepare cells
    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();
    let outputs = vec![
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script.clone())
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script)
            .build(),
    ];

    let outputs_data = vec![Bytes::new(); 2];

    // build transaction
    let tx = TransactionBuilder::default()
        .input(input)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(lock_script_dep)
        .cell_dep(secp256k1_dep)
        .cell_dep(secp256k1_data_dep)
        .build();
    let tx = context.complete_tx(tx);

    // sign
    let tx = sign_tx(tx, &privkey);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_sign_with_wrong_key() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = blake160(&pubkey.serialize());
    let wrong_privkey = Generator::random_privkey();

    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("order-book-contract");
    let out_point = context.deploy_cell(contract_bin);

    let secp256k1_bin: Bytes =
        fs::read("../ckb-miscellaneous-scripts/build/secp256k1_blake2b_sighash_all_dual")
            .expect("load secp256k1")
            .into();
    let secp256k1_out_point = context.deploy_cell(secp256k1_bin);
    let secp256k1_dep = CellDep::new_builder()
        .out_point(secp256k1_out_point)
        .build();

    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();

    // prepare scripts
    let lock_script = context
        .build_script(&out_point, pubkey_hash.to_vec().into())
        .expect("script");
    let lock_script_dep = CellDep::new_builder().out_point(out_point).build();

    // prepare cells
    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();
    let outputs = vec![
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script.clone())
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script)
            .build(),
    ];

    let outputs_data = vec![Bytes::new(); 2];

    // build transaction
    let tx = TransactionBuilder::default()
        .input(input)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(lock_script_dep)
        .cell_dep(secp256k1_dep)
        .cell_dep(secp256k1_data_dep)
        .build();
    let tx = context.complete_tx(tx);

    // sign
    let tx = sign_tx(tx, &wrong_privkey);

    // run
    let err = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect_err("pass verification");
    let script_cell_index = 0;
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(6).input_lock_script(script_cell_index)
    );
}

#[test]
fn test_recover_pubkey() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = blake160(&pubkey.serialize());

    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("order-book-contract");
    let out_point = context.deploy_cell(contract_bin);

    let secp256k1_bin: Bytes =
        fs::read("../ckb-miscellaneous-scripts/build/secp256k1_blake2b_sighash_all_dual")
            .expect("load secp256k1")
            .into();
    let secp256k1_out_point = context.deploy_cell(secp256k1_bin);
    let secp256k1_dep = CellDep::new_builder()
        .out_point(secp256k1_out_point)
        .build();

    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();

    // prepare scripts
    let lock_script = context
        .build_script(&out_point, pubkey_hash.to_vec().into())
        .expect("script");
    let lock_script_dep = CellDep::new_builder().out_point(out_point).build();

    // prepare cells
    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();
    let outputs = vec![
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script.clone())
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script)
            .build(),
    ];

    let outputs_data = vec![Bytes::new(); 2];

    let mut rng = thread_rng();
    let mut message = [0u8; 32];
    rng.fill(&mut message);
    let sig = privkey.sign_recoverable(&message.into()).expect("sign");
    let witness = {
        let mut args = Vec::new();
        args.extend_from_slice(&message);
        args.extend_from_slice(&sig.serialize());
        WitnessArgs::new_builder()
            .input_type(Some(Bytes::from(args)).pack())
            .build()
            .as_bytes()
            .pack()
    };

    // build transaction
    let tx = TransactionBuilder::default()
        .input(input)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(lock_script_dep)
        .cell_dep(secp256k1_dep)
        .cell_dep(secp256k1_data_dep)
        .witness(witness)
        .build();
    let tx = context.complete_tx(tx);

    // run
    let cycles = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
    println!("consume cycles: {}", cycles);
}

#[test]
fn test_recover_pubkey_with_wrong_signature() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let wrong_privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = blake160(&pubkey.serialize());

    // deploy contract
    let mut context = Context::default();
    let contract_bin: Bytes = Loader::default().load_binary("order-book-contract");
    let out_point = context.deploy_cell(contract_bin);

    let secp256k1_bin: Bytes =
        fs::read("../ckb-miscellaneous-scripts/build/secp256k1_blake2b_sighash_all_dual")
            .expect("load secp256k1")
            .into();
    let secp256k1_out_point = context.deploy_cell(secp256k1_bin);
    let secp256k1_dep = CellDep::new_builder()
        .out_point(secp256k1_out_point)
        .build();

    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();

    // prepare scripts
    let lock_script = context
        .build_script(&out_point, pubkey_hash.to_vec().into())
        .expect("script");
    let lock_script_dep = CellDep::new_builder().out_point(out_point).build();

    // prepare cells
    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();
    let outputs = vec![
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script.clone())
            .build(),
        CellOutput::new_builder()
            .capacity(500u64.pack())
            .lock(lock_script)
            .build(),
    ];

    let outputs_data = vec![Bytes::new(); 2];

    let mut rng = thread_rng();
    let mut message = [0u8; 32];
    rng.fill(&mut message);
    let sig = wrong_privkey
        .sign_recoverable(&message.into())
        .expect("sign");
    let witness = {
        let mut args = Vec::new();
        args.extend_from_slice(&message);
        args.extend_from_slice(&sig.serialize());
        WitnessArgs::new_builder()
            .input_type(Some(Bytes::from(args)).pack())
            .build()
            .as_bytes()
            .pack()
    };

    // build transaction
    let tx = TransactionBuilder::default()
        .input(input)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(lock_script_dep)
        .cell_dep(secp256k1_dep)
        .cell_dep(secp256k1_data_dep)
        .witness(witness)
        .build();
    let tx = context.complete_tx(tx);

    // run
    let err = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect_err("pass verification");
    let script_cell_index = 0;
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(6).input_lock_script(script_cell_index)
    );
}
