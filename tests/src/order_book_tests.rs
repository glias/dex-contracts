use super::*;
use ckb_testtool::{builtin::ALWAYS_SUCCESS, context::Context};
use ckb_system_scripts::BUNDLED_CELL;
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

const MAX_CYCLES: u64 = 1000_0000;

fn blake160(data: &[u8]) -> [u8; 20] {
    let mut buf = [0u8; 20];
    let hash = blake2b_256(data);
    buf.clone_from_slice(&hash[..20]);
    buf
}

fn sign_tx(tx: TransactionView, key: &Privkey) -> TransactionView {
    const SIGNATURE_SIZE: usize = 65;

    let witnesses_len = tx.inputs().len();
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
    // (1..witnesses_len).for_each(|n| {
    //     let witness = tx.witnesses().get(n).unwrap();
    //     let witness_len = witness.raw_data().len() as u64;
    //     blake2b.update(&witness_len.to_le_bytes());
    //     blake2b.update(&witness.raw_data());
    // });
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
    same_type: bool,
) -> (Context, TransactionView) {
    // deploy order book script
    let mut context = Context::default();
    let order_bin: Bytes = Loader::default().load_binary("order-book-contract");
    let order_out_point = context.deploy_cell(order_bin);
    
    // deploy always_success script
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    // build lock script
    let type_script = context
        .build_script(&always_success_out_point, Default::default())
        .expect("script");
    let type_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point).build();

    // prepare inputs
    let mut inputs = vec![];
    for index in 0..inputs_token.len() {
        let order_script = context
            .build_script(&order_out_point, input_args.get(index).unwrap().clone())
            .expect("script");
        let token = inputs_token.get(index).unwrap();
        let capacity = Capacity::shannons(*token);
        let input_out_point = context.create_cell(
            CellOutput::new_builder()
                .capacity(capacity.pack())
                .lock(order_script.clone())
                .type_(Some(type_script.clone()).pack())
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
        let token = outputs_token.get(index).unwrap();
        let capacity = Capacity::shannons(*token);
        let order_script = context
            .build_script(&order_out_point, output_args.get(index).unwrap().clone())
            .expect("script");
        let output = if same_type || index != 0 {
            CellOutput::new_builder()
                .capacity(capacity.pack())
                .lock(order_script.clone())
                .type_(Some(type_script.clone()).pack())
                .build()
        } else {
            CellOutput::new_builder()
                .capacity(capacity.pack())
                .lock(order_script.clone())
                .build()
        };
        outputs.push(output);
    }

    let order_script_dep = CellDep::new_builder().out_point(order_out_point).build();
    let mut witnesses = vec![];
    for _ in 0..inputs.len() {
        witnesses.push(Bytes::new())
    }

    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(order_script_dep)
        .cell_dep(type_script_dep)
        .witnesses(witnesses.pack())
        .build();
    (context, tx)
}


#[test]
// Assume the sudt decimal is 8 and the price 5 sudt/ckb
fn test_ckb_sudt_partial_order() {
    // input1: sudt_amount(50sudt 0x12A05F200u128) + order_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input2: sudt_amount(500sudt 0xBA43B7400u128) + order_amount(1000ckb 0x174876E800u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B000000000000000000000000E8764817000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output1: sudt_amount(200sudt 0x4A817C800u128) + order_amount(0sudt 0x12A05F200u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)
    // output2: sudt_amount(349.55sudt 0x8237AF8C0u128) + order_amount(250ckb 0x5D21DBA00u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let outputs_data = vec![
        Bytes::from(
            hex::decode("00C817A80400000000000000000000000000000000000000000000000000000000743BA40B00000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A2308000000000000000000000000BA1DD205000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
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
        true,
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
    // input0: sudt_amount(50sudt 0x12A05F200u128) + order_amount(150sudt 0x37E11D600u128)
    // + price(5.2*10^10 0xC1B710800u64) + buy(00)

    // input1: sudt_amount(500sudt 0xBA43B7400u128) + order_amount(750ckb 0x1176592E00u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000D6117E0300000000000000000000000008711B0C00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B0000000000000000000000002E597611000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output0: sudt_amount(200sudt 0x4A817C800u128) + order_amount(0sudt 0x0u128)
    // + price(5.2*10^10 0xC1B710800u64) + buy(00)

    // output1: sudt_amount(349.55sudt 0x8237AF8C0u128) + order_amount(0ckb 0x0u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let outputs_data = vec![
        Bytes::from(
            hex::decode("00C817A8040000000000000000000000000000000000000000000000000000000008711B0C00000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A230800000000000000000000000000000000000000000000000000000000743BA40B00000001").unwrap()),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
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
        true,
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
    // input0: sudt_amount(0sudt) + order_amount(150sudt) + price(5*10^10) + buy(00)
    // input1: sudt_amount(500sudt) + order_amount(750ckb) + price(5*10^10) + sell(01)
    // input2: sudt_amount(0sudt 0x0u128) + order_amount(50sudt) + price(5*10^10) + buy(00)
    // input3: sudt_amount(100sudt) + order_amount(200ckb) + price(5*10^10) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("0000000000000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B0000000000000000000000002E597611000000000000000000000000743BA40B00000001").unwrap(),
        ),
        Bytes::from(
            hex::decode("0000000000000000000000000000000000286bee00000000000000000000000000743ba40b00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00e40b5402000000000000000000000000c817a804000000000000000000000000743ba40b00000001").unwrap(),
        ),
    ];

    // output0: sudt_amount(150sudt) + order_amount(0sudt) + price(5*10^10) + buy(00)
    // output1: sudt_amount(349.55sudt) + order_amount(0ckb) + price(5*10^10) + sell(01)
    // output2: sudt_amount(40sudt) + order_amount(0sudt) + price(5*10^10) + buy(00)
    // output3: sudt_amount(59.88sudt) + order_amount(0ckb) + price(5*10^10) + sell(01)
    let outputs_data = vec![
        Bytes::from(
            hex::decode("00D6117E0300000000000000000000000000000000000000000000000000000000743BA40B00000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A230800000000000000000000000000000000000000000000000000000000743BA40B00000001").unwrap()),
        Bytes::from(
            hex::decode("00286bee0000000000000000000000000000000000000000000000000000000000743ba40b00000000").unwrap()),
        Bytes::from(
            hex::decode("00a1e9640100000000000000000000000000000000000000000000000000000000743ba40b00000001").unwrap()),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
        Bytes::from(hex::decode("11e5d0105abefa7fcbebf1486dd0a99d5812b793e65cbdb63f4a9b7ab65af719").unwrap()),
        Bytes::from(hex::decode("8d03e403d1e5c44e0b7fa44e98ec2b3da4c20c06f646119324004eec28f62289").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
        Bytes::from(hex::decode("11e5d0105abefa7fcbebf1486dd0a99d5812b793e65cbdb63f4a9b7ab65af719").unwrap()),
        Bytes::from(hex::decode("8d03e403d1e5c44e0b7fa44e98ec2b3da4c20c06f646119324004eec28f62289").unwrap()),
    ];
    // output0 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output1 capacity = 800 + 750 = 1550
    // output2 capacity = 400 - 200 * (1 + 0.003) = 199.4
    // output3 capacity = 400 + 200 = 600
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000, 40000000000, 40000000000],
        vec![124775000000, 155000000000, 19940000000, 60000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
        true,
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
    // input0: sudt_amount(50sudt 0x12A05F200u128) + order_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input1: sudt_amount(500sudt 0xBA43B7400u128) + order_amount(750ckb 0x1176592E00u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B0000000000000000000000002E597611000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output0: sudt_amount(200sudt 0x4A817C800u128) + order_amount(0sudt 0x0u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // output1: sudt_amount(349.55sudt 0x8237AF8C0u128) + order_amount(0ckb 0x0u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let outputs_data = vec![
        Bytes::from(
            hex::decode("00C817A80400000000000000000000000000000000000000000000000000000000743BA40B00000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A230800000000000000000000000000000000000000000000000000000000743BA40B00000001").unwrap()),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    // output0 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output1 capacity = 800 + 740 = 1540 not 1530 (output2 capacity amount is error)
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 153000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
        true,
    );

    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    let script_cell_index = 1;
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(11).input_lock_script(script_cell_index)
    );
}


#[test]
fn test_ckb_sudt_order_type_error() {
    // input0: sudt_amount(50sudt 0x12A05F200u128) + order_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input1: sudt_amount(500sudt 0xBA43B7400u128) + order_amount(1000ckb 0x174876E800u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B000000000000000000000000E8764817000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    // output0: sudt_amount(200sudt 0x4A817C800u128) + order_amount(0sudt 0x12A05F200u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // output1: sudt_amount(349.55sudt 0x8237AF8C0u128) + order_amount(250ckb 0x5D21DBA00u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00) (order type error)
    let outputs_data = vec![
        Bytes::from(
            hex::decode("00C817A80400000000000000000000000000000000000000000000000000000000743BA40B00000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A2308000000000000000000000000BA1DD205000000000000000000000000743BA40B00000000").unwrap(),
        ),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    // output0 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output1 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
        true,
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
    // input0: sudt_amount(50sudt 0x12A05F200u128) + order_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input1: sudt_amount(500sudt 0xBA43B7400u128) + order_amount(1000ckb 0x174876E800u128)
    // + price(6*10^10 0xDF8475800u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B000000000000000000000000E87648170000000000000000000000005847F80D00000001").unwrap(),
        ),
    ];

    // output0: sudt_amount(200sudt 0x4A817C800u128) + order_amount(0sudt 0x12A05F200u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // output1: sudt_amount(349.55sudt 0x8237AF8C0u128) + order_amount(250ckb 0x5D21DBA00u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let outputs_data = vec![
        Bytes::from(
            hex::decode("00C817A80400000000000000000000000000000000000000000000000000000000743BA40B00000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A2308000000000000000000000000BA1DD205000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    // output0 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output1 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
        true,
    );

    let tx = context.complete_tx(tx);

    // run
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    let script_cell_index = 1;
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(17).input_lock_script(script_cell_index)
    );
}

#[test]
fn test_ckb_sudt_all_order_cell_data_format_error() {
    // input0: sudt_amount(50sudt 0x12A05F200u128) + order_amount(150sudt 0x37E11D600u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // input1: sudt_amount(500sudt 0xBA43B7400u128) + order_amount(1000ckb 0x174876E800u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let inputs_data = vec![
        Bytes::from(
            hex::decode("00F2052A01000000000000000000000000D6117E03000000000000000000000000743BA40B00000000").unwrap(),
        ),
        Bytes::from(
            hex::decode("00743BA40B000000000000000000000000E8764817000000000000000000000000743BA40B0000000100").unwrap(),
        ),
    ];

    // output0: sudt_amount(200sudt 0x4A817C800u128) + order_amount(0sudt 0x12A05F200u128)
    // + price(5*10^10 0xBA43B7400u64) + buy(00)

    // output1: sudt_amount(349.55sudt 0x8237AF8C0u128) + order_amount(250ckb 0x5D21DBA00u128)
    // + price(5*10^10 0xBA43B7400u64) + sell(01)
    let outputs_data = vec![
        Bytes::from(
            hex::decode("00C817A80400000000000000000000000000000000000000000000000000000000743BA40B00000000").unwrap()),
        Bytes::from(
            hex::decode("C0F87A2308000000000000000000000000BA1DD205000000000000000000000000743BA40B00000001").unwrap(),
        ),
    ];

    let inputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    let outputs_args = vec![
        Bytes::from(hex::decode("6fe3733cd9df22d05b8a70f7b505d0fb67fb58fb88693217135ff5079713e902").unwrap()),
        Bytes::from(hex::decode("a1b0cb1a3e2c49ff91bfc884a2cb428bae8cac5eea8152629612673cef9d1940").unwrap()),
    ];
    // output0 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output1 capacity = 800 + 750 = 1550
    let (mut context, tx) = build_test_context(
        vec![200000000000, 80000000000],
        vec![124775000000, 155000000000],
        inputs_data,
        outputs_data,
        inputs_args,
        outputs_args,
        true,
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
fn test_cancel_order() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = blake160(&pubkey.serialize());

    // deploy contract
    let mut context = Context::default();
    let order_bin: Bytes = Loader::default().load_binary("order-book-contract");
    let order_out_point = context.deploy_cell(order_bin);

    let secp256k1_bin: Bytes =
        fs::read("../ckb-miscellaneous-scripts/build/secp256k1_blake2b_sighash_all_dual")
            .expect("load secp256k1")
            .into();
    let secp256k1_out_point = context.deploy_cell(secp256k1_bin);

    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();

    let secp256k1_lock_script = context
        .build_script(&secp256k1_out_point, pubkey_hash.to_vec().into())
        .expect("script");
    let secp256k1_dep = CellDep::new_builder()
        .out_point(secp256k1_out_point)
        .build();

    let order_lock_script = context
            .build_script(&order_out_point, secp256k1_lock_script.calc_script_hash().as_bytes())
            .expect("script");
    let order_script_dep = CellDep::new_builder().out_point(order_out_point).build();

    // deploy always_success script
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    // build lock script
    let type_script = context
        .build_script(&always_success_out_point, Default::default())
        .expect("script");
    let type_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point).build();

    // prepare cells
    let mut inputs = vec![];
    let secp256k1_input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(100u64.pack())
            .lock(secp256k1_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let input = CellInput::new_builder()
        .previous_output(secp256k1_input_out_point)
        .build();
    inputs.push(input);

    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(order_lock_script)
            .type_(Some(type_script).pack())
            .build(),
        Bytes::from(
            hex::decode("C0F87A2308000000000000000000000000BA1DD205000000000000000000000000743BA40B00000001").unwrap())
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();
    inputs.push(input);

    let outputs = vec![
        CellOutput::new_builder()
            .capacity(1099u64.pack())
            .lock(secp256k1_lock_script)
            .build(),
    ];

    let outputs_data = vec![Bytes::from(
            hex::decode("C0F87A23080000000000000000000000").unwrap())];

    let mut witnesses = vec![];
    for _ in 0..inputs.len() {
        witnesses.push(Bytes::new())
    }

    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(order_script_dep)
        .cell_dep(secp256k1_dep)
        .cell_dep(secp256k1_data_dep)
        .cell_dep(type_script_dep)
        .witnesses(witnesses.pack())
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
fn test_cancel_order_with_wrong_key() {
    // generate key pair
    let privkey = Generator::random_privkey();
    let pubkey = privkey.pubkey().expect("pubkey");
    let pubkey_hash = blake160(&pubkey.serialize());
    let wrong_privkey = Generator::random_privkey();

    // deploy contract
    let mut context = Context::default();
    let order_bin: Bytes = Loader::default().load_binary("order-book-contract");
    let order_out_point = context.deploy_cell(order_bin);

    let secp256k1_bin: Bytes =
        fs::read("../ckb-miscellaneous-scripts/build/secp256k1_blake2b_sighash_all_dual")
            .expect("load secp256k1")
            .into();
    let secp256k1_out_point = context.deploy_cell(secp256k1_bin);

    let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
    let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
    let secp256k1_data_dep = CellDep::new_builder()
        .out_point(secp256k1_data_out_point)
        .build();

    let secp256k1_lock_script = context
        .build_script(&secp256k1_out_point, pubkey_hash.to_vec().into())
        .expect("script");
    let secp256k1_dep = CellDep::new_builder()
        .out_point(secp256k1_out_point)
        .build();

    let order_lock_script = context
            .build_script(&order_out_point, secp256k1_lock_script.calc_script_hash().as_bytes())
            .expect("script");
    let order_script_dep = CellDep::new_builder().out_point(order_out_point).build();

    // deploy always_success script
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    // build lock script
    let type_script = context
        .build_script(&always_success_out_point, Default::default())
        .expect("script");
    let type_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point).build();

    // prepare cells
    let mut inputs = vec![];
    let secp256k1_input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(100u64.pack())
            .lock(secp256k1_lock_script.clone())
            .build(),
        Bytes::new(),
    );
    let input = CellInput::new_builder()
        .previous_output(secp256k1_input_out_point)
        .build();
    inputs.push(input);

    let input_out_point = context.create_cell(
        CellOutput::new_builder()
            .capacity(1000u64.pack())
            .lock(order_lock_script)
            .type_(Some(type_script).pack())
            .build(),
        Bytes::from(
            hex::decode("C0F87A2308000000000000000000000000BA1DD205000000000000000000000000743BA40B00000001").unwrap())
    );
    let input = CellInput::new_builder()
        .previous_output(input_out_point)
        .build();
    inputs.push(input);

    let outputs = vec![
        CellOutput::new_builder()
            .capacity(1099u64.pack())
            .lock(secp256k1_lock_script)
            .build(),
    ];

    let outputs_data = vec![Bytes::from(
            hex::decode("C0F87A23080000000000000000000000").unwrap())];

    let mut witnesses = vec![];
    for _ in 0..inputs.len() {
        witnesses.push(Bytes::new())
    }

    // build transaction
    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(order_script_dep)
        .cell_dep(secp256k1_dep)
        .cell_dep(secp256k1_data_dep)
        .cell_dep(type_script_dep)
        .witnesses(witnesses.pack())
        .build();
    let tx = context.complete_tx(tx);

    // sign
    let tx = sign_tx(tx, &wrong_privkey);

    let script_cell_index = 0;
    // run
    let err = context
        .verify_tx(&tx, MAX_CYCLES)
        .expect_err("pass verification");
    assert_error_eq!(err, ScriptError::ValidationFailure(-31).input_lock_script(script_cell_index));
}