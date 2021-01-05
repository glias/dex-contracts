use ckb_testtool::builtin::ALWAYS_SUCCESS;
use ckb_testtool::context::Context;
use ckb_tool::ckb_jsonrpc_types::TransactionView;
use ckb_tool::ckb_types::core::TransactionBuilder;
use ckb_tool::ckb_types::packed::{CellDep, CellInput, CellOutput, OutPoint, Script};
use ckb_tool::ckb_types::{bytes::Bytes, prelude::*};

use crate::error::Error;
use crate::types::{Contract, TestInfo};

#[macro_export]
macro_rules! blake2b {
    ($($field: expr), *) => {{
        let mut res = [0u8; 32];
        let mut blake2b = ckb_tool::ckb_hash::new_blake2b();

        $( blake2b.update($field.as_ref()); )*

        blake2b.finalize(&mut res);
        res
    }}
}

pub fn build_lock_context(
    info: &TestInfo,
    contract: Contract,
    tx: TransactionView,
) -> Result<(Context, ckb_tool::ckb_types::core::TransactionView), Error> {
    let contract_name = contract.name.as_str();
    let mut ctx = Context::default();
    let out_point = ctx.deploy_cell(contract.bin.clone());
    let always_success_out_point = ctx.deploy_cell(ALWAYS_SUCCESS.clone());

    let type_script = ctx
        .build_script(&always_success_out_point, Default::default())
        .ok_or_else(|| Error::BuildingScriptErr(contract_name.to_string()))?;
    let type_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point)
        .build();

    let inputs = build_lock_inputs(
        &mut ctx,
        type_script.clone(),
        out_point.clone(),
        info,
        contract_name,
    )?;
    let outputs = build_lock_outputs(
        &mut ctx,
        tx.clone(),
        type_script,
        out_point.clone(),
        contract_name,
    )?;

    let script_dep = CellDep::new_builder().out_point(out_point).build();
    let witnesses = (0..inputs.len()).map(|_| Bytes::new()).collect::<Vec<_>>();

    let outputs_data = tx
        .inner
        .outputs_data
        .into_iter()
        .map(|data| data.into_bytes())
        .collect::<Vec<_>>();

    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(script_dep)
        .cell_dep(type_script_dep)
        .witnesses(witnesses.pack())
        .build();

    Ok((ctx, tx))
}

pub fn build_type_context(
    info: &TestInfo,
    contract: Contract,
    tx: TransactionView,
) -> Result<(Context, ckb_tool::ckb_types::core::TransactionView), Error> {
    let contract_name = contract.name.as_str();
    let mut ctx = Context::default();
    let out_point = ctx.deploy_cell(contract.bin.clone());
    let always_success_out_point = ctx.deploy_cell(ALWAYS_SUCCESS.clone());

    let lock_script = ctx
        .build_script(&always_success_out_point, Default::default())
        .ok_or_else(|| Error::BuildingScriptErr(contract_name.to_string()))?;
    let lock_script_dep = CellDep::new_builder()
        .out_point(always_success_out_point)
        .build();

    let inputs = build_type_inputs(
        &mut ctx,
        lock_script.clone(),
        out_point.clone(),
        info,
        contract_name,
    )?;
    let outputs = build_type_outputs(
        &mut ctx,
        tx.clone(),
        lock_script,
        out_point.clone(),
        contract_name,
    )?;

    let script_dep = CellDep::new_builder().out_point(out_point).build();
    let witnesses = (0..inputs.len()).map(|_| Bytes::new()).collect::<Vec<_>>();

    let outputs_data = tx
        .inner
        .outputs_data
        .into_iter()
        .map(|data| data.into_bytes())
        .collect::<Vec<_>>();

    let tx = TransactionBuilder::default()
        .inputs(inputs)
        .outputs(outputs)
        .outputs_data(outputs_data.pack())
        .cell_dep(script_dep)
        .cell_dep(lock_script_dep)
        .witnesses(witnesses.pack())
        .build();

    Ok((ctx, tx))
}

fn build_lock_inputs(
    ctx: &mut Context,
    type_script: Script,
    out_point: OutPoint,
    info: &TestInfo,
    contract_name: &str,
) -> Result<Vec<CellInput>, Error> {
    let mut inputs = Vec::new();
    for input in info.inputs_info.iter() {
        let lock_script = ctx
            .build_script(&out_point, input.output.lock.args.clone().into_bytes())
            .ok_or_else(|| Error::BuildingScriptErr(contract_name.to_string()))?;
        if let Some(data) = input.data.as_ref() {
            let input_out_point = ctx.create_cell(
                CellOutput::new_builder()
                    .capacity(input.output.capacity.pack())
                    .lock(lock_script)
                    .type_(Some(type_script.clone()).pack())
                    .build(),
                data.content.clone().into_bytes(),
            );

            let input = CellInput::new_builder()
                .previous_output(input_out_point)
                .build();
            inputs.push(input);
        } else {
            return Err(Error::MissingInputData);
        }
    }

    Ok(inputs)
}

fn build_type_inputs(
    ctx: &mut Context,
    lock_script: Script,
    out_point: OutPoint,
    info: &TestInfo,
    contract_name: &str,
) -> Result<Vec<CellInput>, Error> {
    let mut inputs = Vec::new();
    for input in info.inputs_info.iter() {
        let type_script = ctx
            .build_script(
                &out_point,
                input.output.type_.clone().unwrap().args.into_bytes(),
            )
            .ok_or_else(|| Error::BuildingScriptErr(contract_name.to_string()))?;
        if let Some(data) = input.data.as_ref() {
            let input_out_point = ctx.create_cell(
                CellOutput::new_builder()
                    .capacity(input.output.capacity.pack())
                    .lock(lock_script.clone())
                    .type_(Some(type_script).pack())
                    .build(),
                data.content.clone().into_bytes(),
            );

            let input = CellInput::new_builder()
                .previous_output(input_out_point)
                .build();
            inputs.push(input);
        } else {
            return Err(Error::MissingInputData);
        }
    }

    Ok(inputs)
}

fn build_lock_outputs(
    ctx: &mut Context,
    tx: TransactionView,
    type_script: Script,
    out_point: OutPoint,
    contract_name: &str,
) -> Result<Vec<CellOutput>, Error> {
    let mut outputs = Vec::new();
    for cell in tx.inner.outputs.iter() {
        let lock_script = ctx
            .build_script(&out_point, cell.lock.args.clone().into_bytes())
            .ok_or_else(|| Error::BuildingScriptErr(contract_name.to_string()))?;
        let output = CellOutput::new_builder()
            .capacity(cell.capacity.pack())
            .lock(lock_script.clone())
            .type_(Some(type_script.clone()).pack())
            .build();
        outputs.push(output);
    }

    Ok(outputs)
}

fn build_type_outputs(
    ctx: &mut Context,
    tx: TransactionView,
    lock_script: Script,
    out_point: OutPoint,
    contract_name: &str,
) -> Result<Vec<CellOutput>, Error> {
    let mut outputs = Vec::new();
    for cell in tx.inner.outputs.iter() {
        let type_script = ctx
            .build_script(
                &out_point,
                cell.type_.clone().unwrap().args.clone().into_bytes(),
            )
            .ok_or_else(|| Error::BuildingScriptErr(contract_name.to_string()))?;
        let output = CellOutput::new_builder()
            .capacity(cell.capacity.pack())
            .lock(lock_script.clone())
            .type_(Some(type_script).pack())
            .build();
        outputs.push(output);
    }

    Ok(outputs)
}
