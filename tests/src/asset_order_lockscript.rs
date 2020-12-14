use super::*;
use ckb_system_scripts::BUNDLED_CELL;
use ckb_testtool::{builtin::ALWAYS_SUCCESS, context::Context};
use ckb_tool::ckb_crypto::secp::{Generator, Privkey};
use ckb_tool::ckb_error::assert_error_eq;
use ckb_tool::ckb_hash::{blake2b_256, new_blake2b};
use ckb_tool::ckb_script::ScriptError;
use ckb_tool::ckb_types::core::{Capacity, TransactionBuilder, TransactionView};
use ckb_tool::ckb_types::packed::{self, *};
use ckb_tool::ckb_types::{bytes::Bytes, prelude::*, H256};
use generated::cell_data::AssetOrder;
use molecule::prelude::*;

const MAX_CYCLES: u64 = 10000_0000;

const ERR_NEGATIVE_CAPACITY_DIFFERENCE: i8 = 11;
const ERR_PRICE_CHANGED: i8 = 17;
const ERR_ORDER_TYPE_CHANGED: i8 = 28;
// NOTE: This error comes from secp256k1_blake160_sighash_all lock
const ERR_SECP256K1_WRONG_KEY: i8 = -31;

enum OrderType {
    SellCKB = 0,
    BuyCKB = 1,
}

impl OrderType {
    fn to_u8(&self) -> u8 {
        match self {
            OrderType::SellCKB => 0,
            OrderType::BuyCKB => 1,
        }
    }
}

struct OrderCell {
    capacity: Capacity,
    data:     Bytes,
}

impl OrderCell {
    fn builder() -> OrderCellBuilder {
        OrderCellBuilder::default()
    }
}

#[derive(Default)]
struct OrderCellBuilder {
    capacity:       u64,
    sudt_amount:    u128,
    version:        u8,
    order_amount:   u128,
    price_effect:   u64,
    price_exponent: i8,
    order_type:     u8,
}

impl OrderCellBuilder {
    fn capacity(mut self, capacity: u64) -> Self {
        self.capacity = capacity;
        self
    }

    fn capacity_dec(mut self, capacity: u64, decimal: u32) -> Self {
        self.capacity = capacity * 10u64.pow(decimal);
        self
    }

    fn sudt_amount(mut self, sudt_amount: u128) -> Self {
        self.sudt_amount = sudt_amount;
        self
    }

    fn sudt_amount_dec(mut self, sudt_amount: u128, decimal: u32) -> Self {
        self.sudt_amount = sudt_amount * 10u128.pow(decimal);
        self
    }

    fn order_amount(mut self, order_amount: u128) -> Self {
        self.order_amount = order_amount;
        self
    }

    fn order_amount_dec(mut self, order_amount: u128, decimal: u32) -> Self {
        self.order_amount = order_amount * 10u128.pow(decimal);
        self
    }

    fn price(mut self, effect: u64, exponent: i8) -> Self {
        self.price_effect = effect;
        self.price_exponent = exponent;
        self
    }

    fn order_type(mut self, order_type: OrderType) -> Self {
        self.order_type = order_type.to_u8();
        self
    }

    fn build(self) -> OrderCell {
        let version = if self.version == 0 { 1 } else { self.version };
        let price_exponent = self.price_exponent.to_le_bytes();

        let asset_order = AssetOrder::new_builder()
            .sudt_amount(self.sudt_amount.pack())
            .version(version.into())
            .order_amount(self.order_amount.pack())
            .price_effect(self.price_effect.pack())
            .price_exponent(price_exponent[0].into())
            .order_type(self.order_type.into())
            .build();

        OrderCell {
            capacity: Capacity::shannons(self.capacity),
            data:     asset_order.as_bytes(),
        }
    }
}

struct SudtCell {
    capacity: Capacity,
    data:     Bytes,
}

impl SudtCell {
    fn new(capacity: u64, amount: u128) -> Self {
        let sudt_data: Uint128 = amount.pack();

        SudtCell {
            capacity: Capacity::shannons(capacity),
            data:     sudt_data.as_bytes(),
        }
    }

    fn new_with_dec(capacity: u64, cap_dec: u32, amount: u128, amount_dec: u32) -> Self {
        let capacity = capacity * 10u64.pow(cap_dec);
        let sudt_data: Uint128 = (amount * 10u128.pow(amount_dec)).pack();

        SudtCell {
            capacity: Capacity::shannons(capacity),
            data:     sudt_data.as_bytes(),
        }
    }
}

struct FreeCell {
    capacity: Capacity,
}

impl FreeCell {
    fn new(capacity: u64) -> Self {
        FreeCell {
            capacity: Capacity::shannons(capacity),
        }
    }
}

enum OrderInput {
    Order {
        cell:      OrderCell,
        lock_args: Option<Bytes>,
        witness:   Option<Bytes>,
    },
    AnyUnlock {
        cell_deps: Option<Vec<CellDep>>,
        cell:      FreeCell,
        lock:      Script,
        witness:   Bytes,
    },
}

impl OrderInput {
    pub fn new_order(cell: OrderCell) -> Self {
        OrderInput::Order {
            cell,
            lock_args: None,
            witness: None,
        }
    }
}

enum OrderOutput {
    PartialFilledOrder(OrderCell),
    Sudt(SudtCell),
    #[allow(dead_code)]
    Free(FreeCell),
}

fn build_tx(
    context: &mut Context,
    input_orders: Vec<OrderInput>,
    output_results: Vec<OrderOutput>,
) -> TransactionView {
    // Deploy asset order lockscript
    let asset_lock_bin: Bytes = Loader::default().load_binary("asset-order-lockscript");
    let asset_lock_out_point = context.deploy_cell(asset_lock_bin);
    let asset_lock_dep = CellDep::new_builder()
        .out_point(asset_lock_out_point.clone())
        .build();

    // Deploy always sucess script
    let always_success_out_point = context.deploy_cell(ALWAYS_SUCCESS.clone());
    let always_success_dep = CellDep::new_builder()
        .out_point(always_success_out_point.clone())
        .build();

    // Always success lock script
    let always_success_lock_script = context
        .build_script(&always_success_out_point, Default::default())
        .expect("always success lock script");

    // Use always success as test sudt type contract
    let sudt_type_script = always_success_lock_script;

    // Pass idx as args to always success lock script to mock different user lock script
    let create_user_lock_script = |context: &mut Context, idx: usize| -> (Script, Bytes) {
        let user_lock_script = {
            let args = Bytes::from(idx.to_le_bytes().to_vec());
            context
                .build_script(&always_success_out_point, args)
                .expect("user lock script")
        };
        let hash = user_lock_script.calc_script_hash().as_bytes();
        (user_lock_script, hash)
    };

    // Prepare inputs
    let mut inputs = vec![];
    let mut witnesses = vec![];
    let mut cell_deps: Vec<CellDep> = vec![];
    for (idx, order_input) in input_orders.into_iter().enumerate() {
        match order_input {
            OrderInput::Order {
                cell,
                lock_args: opt_args,
                witness: opt_witness,
            } => {
                let hash = opt_args.unwrap_or_else(|| {
                    let (_, hash) = create_user_lock_script(context, idx);
                    hash
                });

                let asset_lock_script = context
                    .build_script(&asset_lock_out_point, hash)
                    .expect("asset lock script");

                let input_out_point = context.create_cell(
                    CellOutput::new_builder()
                        .capacity(cell.capacity.pack())
                        .lock(asset_lock_script.clone())
                        .type_(Some(sudt_type_script.clone()).pack())
                        .build(),
                    cell.data,
                );

                let input = CellInput::new_builder()
                    .previous_output(input_out_point)
                    .build();

                inputs.push(input);
                witnesses.push(opt_witness.unwrap_or_default());
            }
            OrderInput::AnyUnlock {
                cell_deps: opt_cell_deps,
                cell,
                lock,
                witness,
            } => {
                let input_out_point = context.create_cell(
                    CellOutput::new_builder()
                        .capacity(cell.capacity.pack())
                        .lock(lock)
                        .build(),
                    Bytes::new(),
                );

                let input = CellInput::new_builder()
                    .previous_output(input_out_point)
                    .build();

                cell_deps.extend(opt_cell_deps.unwrap_or_default());
                inputs.push(input);
                witnesses.push(witness);
            }
        }
    }

    let mut outputs = vec![];
    let mut outputs_data = vec![];
    for (idx, order_result) in output_results.into_iter().enumerate() {
        let (user_lock_script, hash) = create_user_lock_script(context, idx);

        let (output, data) = match order_result {
            OrderOutput::PartialFilledOrder(order) => {
                let asset_lock_script = context
                    .build_script(&asset_lock_out_point, hash)
                    .expect("asset lock script");

                let output = CellOutput::new_builder()
                    .capacity(order.capacity.pack())
                    .type_(Some(sudt_type_script.clone()).pack())
                    .lock(asset_lock_script)
                    .build();

                (output, order.data)
            }
            OrderOutput::Sudt(sudt) => {
                let output = CellOutput::new_builder()
                    .capacity(sudt.capacity.pack())
                    .type_(Some(sudt_type_script.clone()).pack())
                    .lock(user_lock_script)
                    .build();

                (output, sudt.data)
            }
            OrderOutput::Free(free) => {
                let output = CellOutput::new_builder()
                    .capacity(free.capacity.pack())
                    .lock(user_lock_script)
                    .build();
                (output, Bytes::new())
            }
        };

        outputs.push(output);
        outputs_data.push(data);
    }

    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(asset_lock_dep)
        .cell_dep(always_success_dep)
        .cell_deps(cell_deps)
        .witnesses(witnesses.pack())
        .build();

    tx
}

fn build_test_context(
    input_orders: Vec<OrderInput>,
    output_results: Vec<OrderOutput>,
) -> (Context, TransactionView) {
    let mut context = Context::default();
    let tx = build_tx(&mut context, input_orders, output_results);
    (context, tx)
}

#[test]
fn test_ckb_sudt_two_orders_one_partial_filled_and_one_completed() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity(2000_00_000_000)          // 2000 ckb
        .sudt_amount(50_00_000_000)         // 50 sudt
        .order_amount(150_00_000_000)       // 150 sudt
        .price(5, 0)                        // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity(800_00_000_000)           // 800 ckb
        .sudt_amount(500_00_000_000)        // 500 sudt
        .order_amount(1000_00_000_000)      // 1000 ckb
        .price(5, 0)                        // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 750 = 1550
    let output0 = OrderOutput::Sudt(SudtCell::new(1247_75_000_000, 200_00_000_000));
    let output1 = OrderOutput::PartialFilledOrder(
        OrderCell::builder()
            .capacity(1550_00_000_000)
            .sudt_amount(34955_000_000)
            .order_amount(250_00_000_000)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let (mut context, tx) = build_test_context(vec![input0, input1], vec![output0, output1]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_ckb_sudt_completed_matched_order_pair() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(50, 8)         // 50 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(52, -1)                  // 5.2
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(800, 8)           // 800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(750, 8)       // 750 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let output0 = OrderOutput::Sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));
    let output1 = OrderOutput::Sudt(SudtCell::new_with_dec(1550, 8, 349_55, 6));

    let (mut context, tx) = build_test_context(vec![input0, input1], vec![output0, output1]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

// TODO: random
#[test]
fn test_ckb_sudt_two_completed_matched_order_pairs() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount(0)                 // 0 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(800, 8)           // 800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(750, 8)       // 750 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let input2 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(400, 8)           // 400 ckb
        .sudt_amount(0)                 // 0 sudt
        .order_amount_dec(50, 8)        // 50 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input3 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(400, 8)           // 400 ckb
        .sudt_amount_dec(100, 8)        // 100 sudt
        .order_amount_dec(200, 8)       // 200 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let output0 = OrderOutput::Sudt(SudtCell::new_with_dec(1247_75, 6, 150, 8));
    let output1 = OrderOutput::Sudt(SudtCell::new_with_dec(1550, 8, 349_55, 6));
    let output2 = OrderOutput::Sudt(SudtCell::new_with_dec(199_40, 6, 50, 8));
    let output3 = OrderOutput::Sudt(SudtCell::new_with_dec(600, 8, 59_88, 6));

    let (mut context, tx) = build_test_context(vec![input0, input1, input2, input3], vec![
        output0, output1, output2, output3,
    ]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_ckb_sudt_buy_ckb_negative_capacity_difference() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(50, 8)         // 50 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Pass wrong capacity to trigger NegativeCapacityDifference failed
    // Right capacity is 800 ckb
    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(1800, 8)          // 1800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(750, 8)       // 750 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let output0 = OrderOutput::Sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));
    let output1 = OrderOutput::Sudt(SudtCell::new_with_dec(1550, 8, 349_55, 6));

    let (mut context, tx) = build_test_context(vec![input0, input1], vec![output0, output1]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(ERR_NEGATIVE_CAPACITY_DIFFERENCE).input_lock_script(1)
    );
}

#[test]
fn test_ckb_sudt_sell_ckb_negative_capacity_difference() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(50, 8)         // 50 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(800, 8)           // 800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(750, 8)       // 750 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    // Pass wrong capacity to trigger NegativeCapacityDifference failed
    // Right capacity is 1247.75 ckb
    let output0 = OrderOutput::Sudt(SudtCell::new_with_dec(2247_75, 6, 200, 8));
    let output1 = OrderOutput::Sudt(SudtCell::new_with_dec(1550, 8, 349_55, 6));

    let (mut context, tx) = build_test_context(vec![input0, input1], vec![output0, output1]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(ERR_NEGATIVE_CAPACITY_DIFFERENCE).input_lock_script(0)
    );
}

#[test]
fn test_ckb_sudt_buy_ckb_order_type_changed() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(50, 8)         // 50 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(800, 8)           // 800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(1000, 8)      // 1000 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let output0 = OrderOutput::Sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));
    let output1 = OrderOutput::PartialFilledOrder(
        OrderCell::builder()
            .capacity_dec(1550, 8)
            .sudt_amount_dec(34955, 6)
            .order_amount_dec(250, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build(),
    );

    let (mut context, tx) = build_test_context(vec![input0, input1], vec![output0, output1]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(ERR_ORDER_TYPE_CHANGED).input_lock_script(1)
    );
}

#[test]
fn test_ckb_sudt_order_price_changed() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(50, 8)         // 50 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(800, 8)           // 800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(1000, 8)      // 1000 ckb
        .price(6, 0)                    // 6
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let output0 = OrderOutput::Sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));
    let output1 = OrderOutput::PartialFilledOrder(
        OrderCell::builder()
            .capacity_dec(1550, 8)
            .sudt_amount_dec(34955, 6)
            .order_amount_dec(250, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let (mut context, tx) = build_test_context(vec![input0, input1], vec![output0, output1]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(ERR_PRICE_CHANGED).input_lock_script(1)
    );
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

    let unlock_input = OrderInput::AnyUnlock {
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
            cell,
            lock_args: Some(secp256k1_lock_script.calc_script_hash().as_bytes()),
            witness: None,
        }
    };

    let output = OrderOutput::Sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![unlock_input, order_input], vec![output]);
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

    let unlock_input = OrderInput::AnyUnlock {
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
            cell,
            lock_args: Some(secp256k1_lock_script.calc_script_hash().as_bytes()),
            witness: None,
        }
    };

    let output = OrderOutput::Sudt(SudtCell::new_with_dec(1020, 8, 0, 0));
    let tx = build_tx(&mut context, vec![unlock_input, order_input], vec![output]);
    let tx = context.complete_tx(tx);

    let tx = Secp256k1Lock::sign_tx(tx, &wrong_privkey);
    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(
        err,
        ScriptError::ValidationFailure(ERR_SECP256K1_WRONG_KEY).input_lock_script(0)
    );
}

struct Secp256k1Lock;

impl Secp256k1Lock {
    fn deploy(context: &mut Context) -> (OutPoint, Vec<CellDep>) {
        let secp256k1_lock_bin = BUNDLED_CELL
            .get("specs/cells/secp256k1_blake160_sighash_all")
            .unwrap();
        let secp256k1_lock_out_point = context.deploy_cell(secp256k1_lock_bin.to_vec().into());
        let secp256k1_lock_dep = CellDep::new_builder()
            .out_point(secp256k1_lock_out_point.clone())
            .build();

        let secp256k1_data_bin = BUNDLED_CELL.get("specs/cells/secp256k1_data").unwrap();
        let secp256k1_data_out_point = context.deploy_cell(secp256k1_data_bin.to_vec().into());
        let secp256k1_data_dep = CellDep::new_builder()
            .out_point(secp256k1_data_out_point)
            .build();

        (secp256k1_lock_out_point, vec![
            secp256k1_lock_dep,
            secp256k1_data_dep,
        ])
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
}

fn blake160(data: &[u8]) -> [u8; 20] {
    let mut buf = [0u8; 20];
    let hash = blake2b_256(data);
    buf.clone_from_slice(&hash[..20]);
    buf
}
