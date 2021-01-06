use super::*;

use std::collections::HashMap;

use ckb_dyn_lock::locks::binary::{self, Binary};
use ckb_standalone_debugger::transaction::{
    MockCellDep, MockInfo, MockInput, MockTransaction, ReprMockTransaction,
};
use ckb_system_scripts::BUNDLED_CELL;
use ckb_testtool::{builtin::ALWAYS_SUCCESS, context::Context};
use ckb_tool::ckb_crypto::secp::{Generator, Privkey, Pubkey};
use ckb_tool::ckb_error::assert_error_eq;
use ckb_tool::ckb_hash::{blake2b_256, new_blake2b};
use ckb_tool::ckb_script::{ScriptError, TransactionScriptError};
use ckb_tool::ckb_types::core::{Capacity, DepType, TransactionBuilder, TransactionView};
use ckb_tool::ckb_types::packed::{self, *};
use ckb_tool::ckb_types::{bytes::Bytes, prelude::*, H256};
use ckb_x64_simulator::RunningSetup;
use molecule::prelude::*;
use schema::cell_data::AssetOrder;
use serde_json::to_string_pretty;

const MAX_CYCLES: u64 = 10000_0000;

// Simple macro to setup simulator for coverge
macro_rules! test_contract {
    ($case_name:ident, $body:expr) => {
        #[test]
        fn $case_name() {
            let (context, tx) = $body;

            let setup = RunningSetup {
                is_lock_script:  true,
                is_output:       false,
                script_index:    0,
                native_binaries: HashMap::default(),
            };

            write_native_setup(
                stringify!($case_name),
                "asset-order-lockscript-sim",
                &tx,
                &context,
                &setup,
            );
        }
    };
}

mod cancellation;
mod order_validator;

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

    fn new_unchecked(capacity: u64, data: Bytes) -> OrderCell {
        let capacity = Capacity::shannons(capacity);

        OrderCell { capacity, data }
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

    fn order_amount_dec(mut self, order_amount: u128, decimal: u32) -> Self {
        self.order_amount = order_amount * 10u128.pow(decimal);
        self
    }

    fn order_amount(mut self, order_amount: u128) -> Self {
        self.order_amount = order_amount;
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

    fn order_type_unchecked(mut self, order_type: u8) -> Self {
        self.order_type = order_type;
        self
    }

    fn version(mut self, version: u8) -> Self {
        self.version = version;
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
    #[allow(dead_code)]
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

    fn new_unchecked(capacity: u64, data: Bytes) -> Self {
        SudtCell {
            capacity: Capacity::shannons(capacity),
            data,
        }
    }
}

struct FreeCell {
    capacity: Capacity,
    data:     Bytes,
}

impl FreeCell {
    fn new(capacity: u64) -> Self {
        FreeCell {
            capacity: Capacity::shannons(capacity),
            data:     Bytes::new(),
        }
    }

    fn new_with_dec(capacity: u64, cap_dec: u32) -> Self {
        let capacity = capacity * 10u64.pow(cap_dec);

        FreeCell {
            capacity: Capacity::shannons(capacity),
            data:     Bytes::new(),
        }
    }

    fn new_unchecked(capacity: u64, data: Bytes) -> Self {
        FreeCell {
            capacity: Capacity::shannons(capacity),
            data,
        }
    }
}

enum OrderInput {
    Order {
        cell_deps:        Option<Vec<CellDep>>,
        cell:             OrderCell,
        custom_lock_args: Option<Bytes>,
        witness:          Option<Bytes>,
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
            cell_deps: None,
            cell,
            custom_lock_args: None,
            witness: None,
        }
    }
}

enum OutputCell {
    PartialFilledOrder(OrderCell),
    Sudt(SudtCell),
    Free(FreeCell),
}

struct OrderOutput {
    cell:             OutputCell,
    custom_type_args: Option<Bytes>,
    custom_lock_args: Option<Bytes>,
}

impl OrderOutput {
    fn new_order(cell: OrderCell) -> Self {
        Self::inner_new(OutputCell::PartialFilledOrder(cell))
    }

    fn new_sudt(cell: SudtCell) -> Self {
        Self::inner_new(OutputCell::Sudt(cell))
    }

    fn new_free(cell: FreeCell) -> Self {
        Self::inner_new(OutputCell::Free(cell))
    }

    fn inner_new(cell: OutputCell) -> Self {
        OrderOutput {
            cell,
            custom_type_args: None,
            custom_lock_args: None,
        }
    }

    fn custom_type_args(mut self, args: Bytes) -> Self {
        self.custom_type_args = Some(args);
        self
    }

    fn custom_lock_args(mut self, args: Bytes) -> Self {
        self.custom_lock_args = Some(args);
        self
    }
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
                cell_deps: opt_cell_deps,
                cell,
                custom_lock_args: opt_custom_lock_args,
                witness: opt_witness,
            } => {
                let hash = opt_custom_lock_args.unwrap_or_else(|| {
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

                cell_deps.extend(opt_cell_deps.unwrap_or_default());
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

        let user_lock_script = match order_result.custom_lock_args {
            Some(lock_args) => user_lock_script.as_builder().args(lock_args.pack()).build(),
            None => user_lock_script,
        };

        let sudt_type_script = match order_result.custom_type_args {
            Some(type_args) => {
                let type_script = sudt_type_script.clone();
                type_script.as_builder().args(type_args.pack()).build()
            }
            None => sudt_type_script.clone(),
        };

        let (output, data) = match order_result.cell {
            OutputCell::PartialFilledOrder(order) => {
                let asset_lock_script = context
                    .build_script(&asset_lock_out_point, hash)
                    .expect("asset lock script");

                let output = CellOutput::new_builder()
                    .capacity(order.capacity.pack())
                    .type_(Some(sudt_type_script).pack())
                    .lock(asset_lock_script)
                    .build();

                (output, order.data)
            }
            OutputCell::Sudt(sudt) => {
                let output = CellOutput::new_builder()
                    .capacity(sudt.capacity.pack())
                    .type_(Some(sudt_type_script).pack())
                    .lock(user_lock_script)
                    .build();

                (output, sudt.data)
            }
            OutputCell::Free(free) => {
                let output = CellOutput::new_builder()
                    .capacity(free.capacity.pack())
                    .lock(user_lock_script)
                    .build();

                (output, free.data)
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

fn tx_error(error_code: i8, input_index: usize) -> TransactionScriptError {
    ScriptError::ValidationFailure(error_code).input_lock_script(input_index)
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
                .clone()
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

    fn blake160(data: &[u8]) -> [u8; 20] {
        let mut buf = [0u8; 20];
        let hash = blake2b_256(data);
        buf.clone_from_slice(&hash[..20]);
        buf
    }
}

struct DynLock;

impl DynLock {
    fn deploy(context: &mut Context) -> (OutPoint, Vec<CellDep>) {
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

        (secp256k1_keccak256_out_point, vec![
            secp256k1_data_dep,
            secp256k1_keccak256_dep,
        ])
    }

    fn eth_pubkey(pubkey: Pubkey) -> Bytes {
        use sha3::{Digest, Keccak256};

        let prefix_key: [u8; 65] = {
            let mut temp = [4u8; 65];
            temp[1..65].copy_from_slice(pubkey.as_bytes());
            temp
        };
        let pubkey = secp256k1::key::PublicKey::from_slice(&prefix_key).unwrap();
        let message = Vec::from(&pubkey.serialize_uncompressed()[1..]);

        let mut hasher = Keccak256::default();
        hasher.input(&message);
        Bytes::copy_from_slice(&hasher.result()[12..32])
    }
}

fn create_test_folder(name: &str) -> PathBuf {
    let mut path = TX_FOLDER.clone();
    path.push(&name);
    fs::create_dir_all(&path).expect("create folder");
    path
}

fn build_mock_transaction(tx: &TransactionView, context: &Context) -> MockTransaction {
    let mock_inputs = tx
        .inputs()
        .into_iter()
        .map(|input| {
            let (output, data) = context
                .get_cell(&input.previous_output())
                .expect("get cell");
            MockInput {
                input,
                output,
                data,
                header: None,
            }
        })
        .collect();
    let mock_cell_deps = tx
        .cell_deps()
        .into_iter()
        .map(|cell_dep| {
            if cell_dep.dep_type() == DepType::DepGroup.into() {
                panic!("Implement dep group support later!");
            }
            let (output, data) = context.get_cell(&cell_dep.out_point()).expect("get cell");
            MockCellDep {
                cell_dep,
                output,
                data,
                header: None,
            }
        })
        .collect();
    let mock_info = MockInfo {
        inputs:      mock_inputs,
        cell_deps:   mock_cell_deps,
        header_deps: vec![],
    };
    MockTransaction {
        mock_info,
        tx: tx.data(),
    }
}

fn write_native_setup(
    test_name: &str,
    binary_name: &str,
    tx: &TransactionView,
    context: &Context,
    setup: &RunningSetup,
) {
    let folder = create_test_folder(test_name);
    let mock_tx = build_mock_transaction(&tx, &context);
    let repr_tx: ReprMockTransaction = mock_tx.into();
    let tx_json = to_string_pretty(&repr_tx).expect("serialize to json");
    fs::write(folder.join("tx.json"), tx_json).expect("write tx to local file");
    let setup_json = to_string_pretty(setup).expect("serialize to json");
    fs::write(folder.join("setup.json"), setup_json).expect("write setup to local file");
    fs::write(
        folder.join("cmd"),
        format!(
            "CKB_TX_FILE=\"{}\" CKB_RUNNING_SETUP=\"{}\" \"{}\"",
            folder.join("tx.json").to_str().expect("utf8"),
            folder.join("setup.json").to_str().expect("utf8"),
            Loader::default().path(binary_name).to_str().expect("utf8")
        ),
    )
    .expect("write cmd to local file");
}
