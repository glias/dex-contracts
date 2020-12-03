use core::result::Result;

// Import heap related library from `alloc`
// https://doc.rust-lang.org/alloc/index.html
use alloc::vec::Vec;

// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
use share::ckb_std::{
    ckb_constants::Source,
    ckb_types::{
        packed::{Byte, CellOutput},
        prelude::*,
    },
    // debug,
    high_level::{
        load_cell, load_cell_data, load_cell_lock, load_cell_lock_hash, load_cell_type_hash,
        load_transaction, QueryIter,
    },
};
use share::error::Error;
use share::get_cell_type_hash;
use share::hash::new_blake2b;

use crate::entry::liquidity::{
    AssetOrderCellData, InfoCellData, LiquidityOrderCellData, OrderKind,
};
use crate::entry::parse::{
    parse_asset_order_cell_data, parse_info_cell_data, parse_liquidity_order_cell_data,
    parse_pool_cell_data, parse_sudt_cell_data,
};
use crate::entry::type_id::verify_type_id;

const LIQUIDITY_ORDER_LOCK_CODE_HASH: [u8; 32] = [0u8; 32];
const ASSET_ORDER_LOCK_CODE_HASH: [u8; 32] = [1u8; 32];
const INFO_TYPE_CODE_HASH: [u8; 32] = [2u8; 32];
const INFO_LOCK_CODE_HASH: [u8; 32] = [2u8; 32];
const POOL_TYPE_CODE_HASH: [u8; 32] = [3u8; 32];
const POOL_LOCK_CODE_HASH: [u8; 32] = [3u8; 32];

const ASSET_ORDER_FEE: u128 = 0;
const SUDT_CAPACITY: u128 = 10;
const INFO_CAPACITY: u64 = 15;
const POOL_BASE_CAPACITY: u64 = 20;
const INFO_LOCK_ARGS_LEN: usize = 20;
const ORDER_LOCK_ARGS_LEN: usize = 53;

const CKB_STRING: &str = "ckb";

const fn hash_type_data() -> Byte {
    Byte::new(1u8)
}

const fn hash_ckb_sudt_type_hash(sudt_type_hash: &[u8]) -> Vec<u8> {
    let mut hash = [0u8; 32];
    let mut blake2b = new_blake2b();
    blake2b.update(CKB_STRING.as_ref());
    blake2b.update(sudt_type_hash);
    blake2b.finalize(&mut hash);
    hash[..20].to_vec()
}

fn verify_pool_lock(
    info_in_cell: CellOutput,
    pool_in_cell: CellOutput,
    pool_out_cell: CellOutput,
) -> Result<(), Error> {
    if get_cell_type_hash(&info_in_cell)?.unpack() != INFO_TYPE_CODE_HASH
        || info_in_cell.type_().to_opt().unwrap().hash_type() == hash_type_data()
        || info_in_cell.type_().to_opt().unwrap().args().as_slice()
            != pool_in_cell.lock().args().as_slice()
    {
        return Err(Error::InvalidData);
    }

    if get_cell_type_hash(&pool_out_cell)?.unpack() != get_cell_type_hash(&pool_in_cell)?.unpack()
        || pool_out_cell.lock().code_hash().as_slice() != pool_in_cell.lock().code_hash().as_slice()
    {
        return Err(Error::InvalidData);
    }

    Ok(())
}

fn verify_info_type_liquidity_tx() -> Result<(), Error> {
    let info_in_cell = load_cell(0, Source::GroupInput)?;
    let info_in = parse_info_cell_data(&load_cell_data(0, Source::GroupInput)?)?;
    let pool_in_cell = load_cell(1, Source::GroupInput)?;
    let pool_in = parse_pool_cell_data(&load_cell_data(1, Source::GroupInput)?)?;
    let maker_cell = load_cell(2, Source::GroupInput)?;
    let maker = parse_sudt_cell_data(&load_cell_data(2, Source::GroupInput)?)?;
    let info_out_cell = load_cell(0, Source::GroupOutput)?;
    let info_out = parse_info_cell_data(&load_cell_data(0, Source::GroupOutput)?)?;
    let pool_out_cell = load_cell(1, Source::GroupOutput)?;
    let pool_out = parse_pool_cell_data(&load_cell_data(1, Source::GroupOutput)?)?;
    let maker_liquidity_cell = load_cell(2, Source::GroupOutput)?;
    let maker_liquidity = parse_sudt_cell_data(&load_cell_data(2, Source::GroupOutput)?)?;

    verify_pool_lock(info_in_cell, pool_in_cell, pool_out_cell)?;
    verify_liquidity_sudt(maker_liquidity_cell, info_in_cell, pool_in_cell)?;

    let total_liquidity = info_in.total_liquidity;
    let ckb_reserve = info_in.ckb_reserve;
    let token_reserve = info_in.token_reserve;
    let mut ckb_paid = 0;
    let mut ckb_collect = 0;
    let mut token_paid = 0;
    let mut token_collect = 0;
    let mut liquidity_burn = 0;
    let mut liquidity_collect = 0;
    let mut maker_fee_collect = 0;

    for (idx, (order_cell, order_cell_raw_data)) in QueryIter::new(load_cell, Source::GroupInput)
        .zip(QueryIter::new(load_cell_data, Source::GroupInput))
        .enumerate()
        .skip(3)
    {
        let order_cell_data = parse_liquidity_order_cell_data(&order_cell_raw_data)?;
        verify_order(order_cell, order_cell_data, pool_in_cell)?;

        match order_cell_data.kind {
            OrderKind::AddLiquidity => add_liquidity(
                idx,
                order_cell,
                order_cell_data,
                token_reserve,
                ckb_reserve,
                total_liquidity,
                &mut ckb_collect,
                &mut token_collect,
                &mut liquidity_collect,
                &mut maker_fee_collect,
            )?,
            OrderKind::RemoveLiquidity => remove_liquidity(
                idx,
                order_cell,
                info_in_cell,
                pool_in_cell,
                order_cell_data,
                pool_out_cell,
                token_reserve,
                ckb_reserve,
                total_liquidity,
                &mut ckb_paid,
                &mut token_paid,
                &mut liquidity_burn,
                &mut maker_fee_collect,
            )?,
            _ => return Err(Error::InvalidOrderKind),
        }
    }

    if info_out_cell.capacity().unpack() != INFO_CAPACITY
        || info_out.ckb_reserve != (info_in.ckb_reserve - ckb_paid + ckb_collect)
        || info_out.token_reserve != (info_in.token_reserve - token_paid + token_collect)
        || info_out.total_liquidity
            != (info_in.total_liquidity - liquidity_burn + liquidity_collect)
    {
        return Err(Error::InvalidData);
    }

    let pool_out_cell_capcity = pool_out_cell.capacity().unpack() as u128;
    if pool_out_cell_capcity
        != (POOL_BASE_CAPACITY as u128 + info_in.ckb_reserve - ckb_paid + ckb_collect)
        || pool_out_cell_capcity != (POOL_BASE_CAPACITY as u128 + info_out.ckb_reserve)
        || pool_out_cell_capcity
            != (pool_in_cell.capacity().unpack() as u128 - ckb_paid + ckb_collect)
        || pool_out.token_amount != (info_in.token_reserve - token_paid + token_collect)
        || pool_out.token_amount != info_out.token_reserve
        || pool_out.token_amount != (pool_in.token_amount - token_paid + token_collect)
    {
        return Err(Error::InvalidData);
    }

    Ok(())
}

fn add_liquidity(
    order_cell_index: usize,
    order_cell: CellOutput,
    order_cell_data: LiquidityOrderCellData,
    token_reserve: u128,
    ckb_reserve: u128,
    total_liquidity: u128,
    ckb_collect: &mut u128,
    token_collect: &mut u128,
    liquidity_collect: &mut u128,
    maker_fee_collect: &mut u128,
) -> Result<(), Error> {
    let mut min_token_amount = 0;
    let mut min_liquidity = 0;

    if total_liquidity > 0 {
        min_token_amount = order_cell_data.price_ckb_amount * token_reserve / ckb_reserve + 1;
        if min_token_amount > order_cell_data.price_token_amount {
            return Err(Error::AddLiquidityFailed);
        }

        min_liquidity = order_cell_data.price_ckb_amount * total_liquidity / ckb_reserve;
    } else {
        if ckb_reserve != 0 || token_reserve != 0 || total_liquidity != 0 {
            return Err(Error::AddLiquidityFailed);
        }
    }

    if min_liquidity <= order_cell_data.maker_fee {
        return Err(Error::AddLiquidityFailed);
    }

    let liquidity_cell = load_cell(order_cell_index, Source::GroupOutput)?;
    let liquidity_cell_data =
        parse_liquidity_order_cell_data(&load_cell_data(order_cell_index, Source::GroupOutput)?)?;
    if liquidity_cell.lock().code_hash().raw_data().as_ref()
        != order_cell.lock().args().raw_data().as_ref()
        || (liquidity_cell.capacity().unpack() as u128)
            != (order_cell.capacity().unpack() as u128 - order_cell_data.price_ckb_amount)
        || liquidity_cell_data.token_amount != (min_liquidity - order_cell_data.maker_fee)
    {
        return Err(Error::AddLiquidityFailed);
    }

    *ckb_collect += order_cell_data.price_ckb_amount;
    *token_collect += min_token_amount;
    *liquidity_collect += min_liquidity;
    *maker_fee_collect += order_cell_data.maker_fee;

    Ok(())
}

fn remove_liquidity(
    order_cell_index: usize,
    order_cell: CellOutput,
    info_in_cell: CellOutput,
    pool_in_cell: CellOutput,
    order_cell_data: LiquidityOrderCellData,
    pool_out_cell: CellOutput,
    token_reserve: u128,
    ckb_reserve: u128,
    total_liquidity: u128,
    ckb_paid: &mut u128,
    token_paid: &mut u128,
    liquidity_burn: &mut u128,
    maker_fee_collect: &mut u128,
) -> Result<(), Error> {
    if total_liquidity == 0 {
        return Err(Error::RemoveLiquidityFailed);
    }

    verify_liquidity_sudt(pool_out_cell, info_in_cell, pool_in_cell);

    let liquidity_amount = order_cell_data.token_amount - order_cell_data.maker_fee;
    if liquidity_amount == 0 || order_cell_data.token_amount <= order_cell_data.maker_fee {
        return Err(Error::RemoveLiquidityFailed);
    }

    let mut min_ckb_amount = liquidity_amount * ckb_reserve / total_liquidity;
    let mut min_token_amount = liquidity_amount * token_reserve / total_liquidity;

    if min_ckb_amount < order_cell_data.price_token_amount
        || min_token_amount < order_cell_data.price_token_amount
    {
        return Err(Error::RemoveLiquidityFailed);
    }

    let user_refund = load_cell(order_cell_index, Source::GroupOutput)?;
    let user_refund_data =
        parse_sudt_cell_data(&load_cell_data(order_cell_index, Source::GroupOutput)?)?;
    if get_cell_type_hash(&user_refund)?.unpack() != get_cell_type_hash(&pool_out_cell)?.unpack()
        || user_refund.lock().code_hash().raw_data().to_vec()
            != order_cell.lock().args().as_bytes().to_vec()
        || (user_refund.capacity().unpack() as u128)
            != (order_cell.capacity().unpack() as u128 + min_ckb_amount)
        || user_refund_data.amount != min_token_amount
    {
        return Err(Error::RemoveLiquidityFailed);
    }

    *liquidity_burn += liquidity_amount;
    *ckb_paid += min_ckb_amount;
    *token_paid += min_token_amount;
    *maker_fee_collect += order_cell_data.maker_fee;

    if *liquidity_burn > total_liquidity || *ckb_paid > ckb_reserve || *token_paid > token_reserve {
        return Err(Error::RemoveLiquidityFailed);
    }

    Ok(())
}

fn verify_info_type_asset_tx() -> Result<(), Error> {
    let info_in_cell = load_cell(0, Source::GroupInput)?;
    let info_in = parse_info_cell_data(&load_cell_data(0, Source::GroupInput)?)?;
    let pool_in_cell = load_cell(1, Source::GroupInput)?;
    let pool_in = parse_info_cell_data(&load_cell_data(1, Source::GroupInput)?)?;
    let maker_cell = load_cell(2, Source::GroupInput)?;
    let maker = parse_sudt_cell_data(&load_cell_data(2, Source::GroupInput)?)?;
    let info_out_cell = load_cell(0, Source::GroupOutput)?;
    let info_out = parse_info_cell_data(&load_cell_data(0, Source::GroupInput)?)?;
    let pool_out_cell = load_cell(1, Source::GroupOutput)?;
    let pool_out = parse_pool_cell_data(&load_cell_data(1, Source::GroupInput)?)?;
    let maker_sudt_cell = load_cell(2, Source::GroupOutput)?;
    let maker_sudt = parse_sudt_cell_data(&load_cell_data(2, Source::GroupInput)?)?;

    verify_pool_lock(info_in_cell, pool_in_cell, pool_out_cell)?;

    let ckb_reserve = info_in.ckb_reserve;
    let token_reserve = info_in.token_reserve;
    let pool_ckb_obtain = 0;
    let pool_token_paid = 0;
    let maker_fee_ckb_collect = 0;
    let pool_token_obtain = 0;
    let pool_ckb_paid = 0;
    let maker_fee_token_collect = 0;

    if hash_ckb_sudt_type_hash(get_cell_type_hash(&pool_in_cell)?.as_slice())
        == info_in_cell.lock().args().as_slice()
    {
        return Err(Error::InvalidArgument);
    }

    for (idx, (order_cell, order_cell_raw_data)) in QueryIter::new(load_cell, Source::GroupInput)
        .zip(QueryIter::new(load_cell_data, Source::GroupInput))
        .enumerate()
        .skip(3)
    {
        if get_cell_type_hash(&order_cell)?.unpack() != get_cell_type_hash(&pool_in_cell)?.unpack()
        {
            return Err(Error::InvalidTypeHash);
        }

        let order_cell_data = parse_asset_order_cell_data(&order_cell_raw_data)?;
        match order_cell_data.kind {
            OrderKind::SellCkb => sell_sudt(
                idx,
                order_cell,
                pool_out_cell,
                order_cell_data,
                info_in,
                &mut pool_ckb_obtain,
                &mut pool_token_paid,
                &mut maker_fee_ckb_collect,
            )?,

            OrderKind::BuyCkb => buy_sudt(
                idx,
                order_cell,
                pool_out_cell,
                order_cell_data,
                info_in,
                &mut pool_token_obtain,
                &mut pool_ckb_paid,
                &mut maker_fee_token_collect,
                ckb_reserve,
            )?,

            _ => return Err(Error::InvalidOrderKind),
        }
    }

    if info_out_cell.capacity().unpack() != INFO_CAPACITY
        || info_out.ckb_reserve != info_in.ckb_reserve
        || info_out.token_reserve != (info_in.token_reserve - pool_token_paid + pool_token_obtain)
        || info_out.total_liquidity != info_in.total_liquidity
    {
        return Err(Error::InvalidData);
    }

    let pool_out_capacity = pool_out_cell.capacity().unpack() as u128;
    if pool_out_capacity
        != (POOL_BASE_CAPACITY as u128 + info_in.ckb_reserve - pool_ckb_paid + pool_ckb_obtain)
        || pool_out_capacity != (POOL_BASE_CAPACITY as u128 + info_out.ckb_reserve)
        || pool_out.token_amount != (info_in.token_reserve - pool_token_paid + pool_token_obtain)
        || pool_out.token_amount != info_out.token_reserve
        || pool_out.token_amount != (pool_in.token_reserve - pool_token_paid + pool_token_obtain)
    {
        return Err(Error::InvalidData);
    }

    if get_cell_type_hash(&maker_sudt_cell)?.as_slice()
        != get_cell_type_hash(&pool_out_cell)?.as_slice()
        || maker_sudt_cell.capacity().unpack() >= maker_cell.capacity().unpack()
        || maker_sudt.amount != maker_fee_token_collect
    {
        return Err(Error::InvalidData);
    }

    Ok(())
}

fn sell_sudt(
    order_cell_index: usize,
    order_cell: CellOutput,
    pool_cell: CellOutput,
    order: AssetOrderCellData,
    info: InfoCellData,
    pool_ckb_obtain: &mut u128,
    pool_token_paid: &mut u128,
    maker_fee_ckb_collect: &mut u128,
) -> Result<(), Error> {
    let user_token_bought = order.order_amount;
    let user_ckb_paid = (1000 * info.ckb_reserve * user_token_bought) / 997
        * (info.token_reserve - user_token_bought);
    let real_price = user_ckb_paid / user_token_bought;

    if real_price > order.price {
        return Err(Error::InvalidData);
    }

    let maker_fee = user_token_bought * real_price * ASSET_ORDER_FEE;
    if (order_cell.capacity().unpack() as u128) < (maker_fee + SUDT_CAPACITY)
        || user_ckb_paid > (order_cell.capacity().unpack() as u128 - maker_fee - SUDT_CAPACITY)
    {
        return Err(Error::SellCkbFailed);
    }

    let usdt_cell = load_cell(order_cell_index, Source::GroupOutput)?;
    if get_cell_type_hash(&usdt_cell)?.unpack() != get_cell_type_hash(&pool_cell)?.unpack()
        || usdt_cell.lock().code_hash().raw_data().to_vec() != order_cell.lock().args().as_slice()
        || (usdt_cell.capacity().unpack() as u128)
            != (order_cell.capacity().unpack() as u128 - user_ckb_paid - maker_fee)
    {
        return Err(Error::SellCkbFailed);
    }

    *pool_ckb_obtain += user_ckb_paid;
    *pool_token_paid += user_token_bought;
    *maker_fee_ckb_collect += maker_fee;

    Ok(())
}

fn buy_sudt(
    order_cell_index: usize,
    order_cell: CellOutput,
    pool_cell: CellOutput,
    order: AssetOrderCellData,
    info: InfoCellData,
    pool_token_obtain: &mut u128,
    pool_ckb_paid: &mut u128,
    maker_fee_token_collect: &mut u128,
    ckb_reserve: u128,
) -> Result<(), Error> {
    let user_ckb_bought = order.order_amount;
    let user_token_paid =
        (1000 * info.token_reserve * user_ckb_bought) / 997 * (info.ckb_reserve - user_ckb_bought);
    let real_price = user_ckb_bought / user_token_paid;

    if real_price > order.price {
        return Err(Error::InvalidData);
    }

    let maker_fee = user_ckb_bought * ASSET_ORDER_FEE / real_price;
    if user_token_paid > (order.token_amount - maker_fee) {
        return Err(Error::BuyCkbFailed);
    }

    let usdt_cell = load_cell(order_cell_index, Source::GroupOutput)?;
    if get_cell_type_hash(&usdt_cell)?.unpack() != get_cell_type_hash(&pool_cell)?.unpack()
        || usdt_cell.lock().code_hash().raw_data().to_vec() != order_cell.lock().args().as_slice()
        || (usdt_cell.capacity().unpack() as u128)
            != (order_cell.capacity().unpack() as u128
                - user_ckb_bought
                - maker_fee
                - user_token_paid)
    {
        return Err(Error::BuyCkbFailed);
    }

    *pool_ckb_paid += user_ckb_bought;
    *pool_token_obtain += user_token_paid;
    *maker_fee_token_collect += maker_fee;

    if *pool_ckb_paid > ckb_reserve {
        return Err(Error::BuyCkbFailed);
    }

    Ok(())
}

fn verify_liquidity_sudt(
    liquidity_sudt_cell: CellOutput,
    info_cell: CellOutput,
    pool_cell: CellOutput,
) -> Result<(), Error> {
    let liquidity_sudt_type_script = liquidity_sudt_cell
        .type_()
        .to_opt()
        .ok_or(Error::MissingTypeScript)?;

    if hash_ckb_sudt_type_hash(liquidity_sudt_type_script.code_hash().as_slice())
        != info_cell.lock().args().as_slice()
        || liquidity_sudt_type_script.args().as_slice()
            != pool_cell.lock().code_hash().as_bytes().as_ref()
    {
        return Err(Error::InvalidData);
    }

    Ok(())
}

fn verify_order(
    order_cell: CellOutput,
    order_cell_data: LiquidityOrderCellData,
    pool_cell: CellOutput,
) -> Result<(), Error> {
    if get_cell_type_hash(&order_cell)?.as_slice() != get_cell_type_hash(&pool_cell)?.as_slice()
        || order_cell.lock().args().len() != ORDER_LOCK_ARGS_LEN
        || !order_cell_data.kind.is_liquidity_opt()
    {
        return Err(Error::InvalidOrderKind);
    }
    Ok(())
}

fn verify_info_creation() -> Result<(), Error> {
    if QueryIter::new(load_cell_type_hash, Source::GroupInput)
        .map(|script_hash| script_hash == Some(INFO_TYPE_CODE_HASH))
        .count()
        == 0
    {
        let pool_out_cell_indexes = QueryIter::new(load_cell_lock_hash, Source::GroupOutput)
            .enumerate()
            .filter_map(|(idx, script_hash)| {
                if script_hash == POOL_LOCK_CODE_HASH {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if pool_out_cell_indexes.len() != 1 {
            return Err(Error::PoolNotFound);
        }

        let pool_out_cell_lock = load_cell_lock(pool_out_cell_indexes[0], Source::GroupOutput)?;
        let info_out_cell = load_cell(0, Source::GroupOutput)?;
        if pool_out_cell_lock.args().as_slice() != get_cell_type_hash(&info_out_cell)?.as_slice() {
            return Err(Error::InvalidData);
        }

        let pool_out_cell_type_hash =
            load_cell_type_hash(pool_out_cell_indexes[0], Source::GroupOutput)?
                .ok_or(Error::MissingTypeScript)?;
        if hash_ckb_sudt_type_hash(&pool_out_cell_type_hash)
            != info_out_cell.lock().args().as_slice()
        {
            return Err(Error::InvalidData);
        }
    }
    Ok(())
}

fn verify_info_type() -> Result<(), Error> {
    verify_type_id(load_transaction()?)?;
    verify_info_creation()?;

    let info_in_cell = load_cell(0, Source::GroupInput)?;
    let info_in = parse_info_cell_data(&load_cell_data(0, Source::GroupInput)?)?;
    let info_out_cell = load_cell(0, Source::GroupOutput)?;
    let info_out = parse_info_cell_data(&load_cell_data(0, Source::GroupOutput)?)?;

    let info_in_cell_lock = info_in_cell.lock();
    if info_in_cell_lock.code_hash().unpack() != INFO_LOCK_CODE_HASH
        || info_in_cell_lock.hash_type() != hash_type_data()
        || info_in_cell_lock.args().len() != INFO_LOCK_ARGS_LEN
    {
        return Err(Error::InvalidArgument);
    }

    if get_cell_type_hash(&info_in_cell)?.unpack() != get_cell_type_hash(&info_out_cell)?.unpack()
        || info_in_cell.lock().code_hash().unpack() != info_out_cell.lock().code_hash().unpack()
    {
        return Err(Error::InvalidCodeHash);
    }

    if QueryIter::new(load_cell_type_hash, Source::GroupInput)
        .map(|script_hash| script_hash == Some(INFO_TYPE_CODE_HASH))
        .count()
        != 1
        || QueryIter::new(load_cell_type_hash, Source::GroupOutput)
            .map(|script_hash| script_hash == Some(INFO_TYPE_CODE_HASH))
            .count()
            != 1
    {
        return Err(Error::OnlyOneLiquidityPool);
    }

    // Todo: perf
    let liquidity_order_count = QueryIter::new(load_cell_lock_hash, Source::GroupInput)
        .map(|script_hash| script_hash == ASSET_ORDER_LOCK_CODE_HASH)
        .count();
    let asset_order_count = QueryIter::new(load_cell_lock_hash, Source::GroupInput)
        .map(|script_hash| script_hash == LIQUIDITY_ORDER_LOCK_CODE_HASH)
        .count();

    if asset_order_count != 0 && liquidity_order_count != 0
        || (asset_order_count + liquidity_order_count) == 0
    {
        return Err(Error::InvalidData);
    }

    if liquidity_order_count > 0 {
        verify_info_type_liquidity_tx()?;
    } else if asset_order_count > 0 {
        verify_info_type_asset_tx()?;
    } else {
        return Err(Error::InvalidData);
    }

    Ok(())
}

fn verify_info_lock(info_cell: CellOutput) -> Result<(), Error> {
    if get_cell_type_hash(&info_cell)?.unpack() == INFO_TYPE_CODE_HASH
        || info_cell.type_().to_opt().unwrap().hash_type() == hash_type_data()
    {
        return Ok(());
    }

    Err(Error::InvalidData)
}
