use core::convert::TryInto;
use core::result::Result;

use num_bigint::BigUint;

// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
use share::cell::{InfoCellData, LiquidityOrderCellData, PoolCellData, Price, SUDTCellData};
use share::ckb_std::{
    ckb_constants::Source,
    ckb_types::{packed::CellOutput, prelude::*},
    // debug,
    high_level::{load_cell, load_cell_data, QueryIter},
};
use share::error::Error;
use share::{decode_u128, get_cell_type_hash};

const LIQUIDITY_ORDER_LOCK_CODE_HASH: [u8; 32] = [0u8; 32];
const ASSET_ORDER_LOCK_CODE_HASH: [u8; 32] = [1u8; 32];

const POOL_TYPE_CODE_HASH: [u8; 32] = [3u8; 32];
const POOL_LOCK_CODE_HASH: [u8; 32] = [3u8; 32];
const INFO_VERSION: u8 = 0;

const THOUSAND: u128 = 1_000;
const TEN_THOUSAND: u128 = 10_000;
const ASSET_ORDER_FEE: u128 = 0;
const SUDT_CAPACITY: u128 = 10;
const INFO_CAPACITY: u64 = 15;
const POOL_BASE_CAPACITY: u64 = 20;
const INFO_LOCK_ARGS_LEN: usize = 20;
const ORDER_LOCK_ARGS_LEN: usize = 53;

const CKB_STRING: &str = "ckb";

pub fn verify_info_type_liquidity_tx() -> Result<(), Error> {
    let info_in_cell = load_cell(0, Source::Input)?;
    let info_in_data = InfoCellData::from_raw(&load_cell_data(0, Source::Input)?)?;
    let pool_in_cell = load_cell(1, Source::Input)?;
    let pool_in_data = PoolCellData::from_raw(&load_cell_data(1, Source::Input)?)?;
    let matcher_in_cell = load_cell(2, Source::Input)?;
    let matcher_in_data = SUDTCellData::from_raw(&load_cell_data(2, Source::Input)?)?;
    let info_out_cell = load_cell(0, Source::Output)?;
    let info_out_data = InfoCellData::from_raw(&load_cell_data(0, Source::Output)?)?;
    let pool_out_cell = load_cell(1, Source::Output)?;
    let pool_out_data = PoolCellData::from_raw(&load_cell_data(1, Source::Output)?)?;
    let matcher_out_cell = load_cell(2, Source::Output)?;
    let matcher_out_data = SUDTCellData::from_raw(&load_cell_data(2, Source::Output)?)?;

    let mut liquidity_sudt_type_hash = [0u8; 20];
    liquidity_sudt_type_hash.copy_from_slice(&info_in_data.liquidity_sudt_type_hash);
    let mut pool_type_hash = [0u8; 32];
    pool_type_hash.copy_from_slice(&get_cell_type_hash(&pool_in_cell)?.unpack()[0..20]);

    let ckb_reserve = info_in_data.ckb_reserve;
    let sudt_reserve = info_in_data.sudt_reserve;
    let total_liquidity = info_in_data.total_liquidity;

    let mut pool_ckb_paid = 0;
    let mut pool_sudt_paid = 0;
    let mut ckb_collect = 0;
    let mut sudt_collect = 0;
    let mut user_liquidity_mint = 0;
    let mut user_liquidity_burned = 0;
    let mut base_index = 0;

    for (idx, (liquidity_order_cell, raw_data)) in QueryIter::new(load_cell, Source::Input)
        .zip(QueryIter::new(load_cell_data, Source::Input))
        .enumerate()
        .skip(3)
    {
        let liquidity_order_data = LiquidityOrderCellData::from_raw(&raw_data)?;
        if liquidity_order_data.version != INFO_VERSION {
            return Err(Error::VersionDiff);
        }

        let liquidity_code_hash = get_cell_type_hash(&liquidity_order_cell)?.unpack();
        if liquidity_order_data.price.coefficient == 0
            || liquidity_code_hash[0..20] != get_cell_type_hash(&info_in_cell)?.unpack()[0..20]
        {
            return Err(Error::InvalidLiquidityCell);
        }

        match &liquidity_code_hash[0..20] {
            liquidity_sudt_type_hash => {
                burn_liquidity(
                    idx,
                    &liquidity_order_cell,
                    &liquidity_order_data,
                    &pool_out_cell,
                    ckb_reserve,
                    sudt_reserve,
                    total_liquidity,
                    &mut pool_ckb_paid,
                    &mut pool_sudt_paid,
                    &mut user_liquidity_burned,
                )?;
                base_index = idx;
            }

            pool_type_hash => mint_liquidity(
                base_index,
                idx,
                &info_out_data,
                &pool_out_cell,
                &liquidity_order_cell,
                &liquidity_order_data,
                ckb_reserve,
                sudt_reserve,
                total_liquidity,
                &mut ckb_collect,
                &mut sudt_collect,
                &mut user_liquidity_mint,
            )?,

            _ => return Err(Error::UnknownLiquidity),
        }
    }

    if info_out_cell.capacity().unpack() != INFO_CAPACITY
        || BigUint::from(info_out_data.ckb_reserve)
            != (BigUint::from(info_in_data.ckb_reserve) - pool_ckb_paid + ckb_collect)
        || BigUint::from(info_out_data.sudt_reserve)
            != (BigUint::from(info_in_data.sudt_reserve) - pool_sudt_paid + sudt_collect)
        || BigUint::from(info_out_data.total_liquidity)
            >= (BigUint::from(info_in_data.total_liquidity) * TEN_THOUSAND * 9985u128
                - BigUint::from(user_liquidity_burned)
                    * 9985u128
                    * 9985u128
                    * user_liquidity_burned
                + BigUint::from(user_liquidity_mint) * TEN_THOUSAND * TEN_THOUSAND)
    {
        return Err(Error::InvalidData);
    }

    if (pool_out_cell.capacity().unpack() as u128)
        != (pool_in_cell.capacity().unpack() as u128 + info_out_data.ckb_reserve
            - info_in_data.ckb_reserve)
        || pool_out_data.sudt_amount != info_out_data.sudt_reserve
    {
        return Err(Error::InvalidData);
    }

    Ok(())
}

pub fn verify_info_type_asset_tx() -> Result<(), Error> {
    let info_in_cell = load_cell(0, Source::GroupInput)?;
    let info_in_data = InfoCellData::from_raw(&load_cell_data(0, Source::Input)?)?;
    let pool_in_cell = load_cell(1, Source::GroupInput)?;
    let pool_in_data = PoolCellData::from_raw(&load_cell_data(1, Source::Input)?)?;
    let info_out_cell = load_cell(0, Source::GroupOutput)?;
    let info_out_data = InfoCellData::from_raw(&load_cell_data(0, Source::Input)?)?;
    let pool_out_cell = load_cell(1, Source::GroupOutput)?;
    let pool_out_data = PoolCellData::from_raw(&load_cell_data(1, Source::Input)?)?;

    if info_out_cell.capacity().unpack() != INFO_CAPACITY
        || info_out_data.total_liquidity != info_in_data.total_liquidity
    {
        return Err(Error::InvalidInfoData);
    }

    let ckb_got = info_out_data.ckb_reserve - info_in_data.ckb_reserve;
    let sudt_got = info_out_data.sudt_reserve - info_in_data.sudt_reserve;
    let ckb_reserve = info_in_data.ckb_reserve;
    let sudt_reserve = info_in_data.sudt_reserve;

    if ckb_got > 0 && sudt_got < 0 {
        let sudt_paid = info_in_data.sudt_reserve - info_out_data.sudt_reserve;
        if BigUint::from(ckb_got) * 998u128 * (sudt_reserve - sudt_paid)
            != BigUint::from(ckb_reserve) * sudt_paid * THOUSAND
        {
            return Err(Error::BuySUDTFailed);
        }
    } else if ckb_got < 0 && sudt_got > 0 {
        let ckb_paid = info_in_data.ckb_reserve - info_out_data.ckb_reserve;
        if BigUint::from(sudt_got) * 998u128 * ckb_reserve
            != BigUint::from(ckb_paid) * (sudt_reserve * THOUSAND + 998u128 * sudt_got)
        {
            return Err(Error::SellSUDTFailed);
        }
    }

    if (pool_out_cell.capacity().unpack() as u128)
        != (pool_in_cell.capacity().unpack() as u128) + ckb_got
        || pool_out_data.sudt_amount != pool_in_data.sudt_amount + sudt_got
    {
        return Err(Error::AmountDiff);
    }

    Ok(())
}

fn mint_liquidity(
    base_index: usize,
    liquidity_cell_index: usize,
    info_out_data: &InfoCellData,
    pool_out_cell: &CellOutput,
    liquidity_order_cell: &CellOutput,
    liquidity_order_data: &LiquidityOrderCellData,
    ckb_reserve: u128,
    sudt_reserve: u128,
    total_liquidity: u128,
    ckb_collect: &mut u128,
    sudt_collect: &mut u128,
    user_liquidity_mint: &mut u128,
) -> Result<(), Error> {
    let relative_index = liquidity_cell_index - base_index;
    let liquidity_cell = load_cell(relative_index * 2 + base_index, Source::Output)?;
    let liquidity_cell_data = LiquidityOrderCellData::from_raw(&load_cell_data(
        relative_index * 2 + base_index,
        Source::Output,
    )?)?;
    let user_liquidity = liquidity_cell_data.sudt_amount;

    if total_liquidity > 0 {
        let change_cell = load_cell(relative_index * 2 + base_index + 1, Source::Output)?;
        if get_cell_type_hash(&liquidity_cell)?.unpack()[0..20]
            != info_out_data.liquidity_sudt_type_hash
            || liquidity_cell.lock().code_hash().as_slice()
                != liquidity_order_cell.lock().args().as_slice()
        {
            return Err(Error::InvalidLiquidityCell);
        }

        let change_cell_data = load_cell_data(relative_index * 2 + base_index + 1, Source::Output)?;

        let mut ckb_injected = 0;
        let mut sudt_injected = 0;

        if change_cell_data.len() == 0 {
            if change_cell.type_().to_opt().is_none()
                || change_cell.lock().code_hash().as_slice()
                    != liquidity_order_cell.lock().args().as_slice()
            {
                return Err(Error::InvalidChangeCell);
            }

            sudt_injected = liquidity_order_data.sudt_amount;
            ckb_injected = (liquidity_order_cell.capacity().unpack() as u128)
                - SUDT_CAPACITY
                - (change_cell.capacity().unpack() as u128);

            if BigUint::from(user_liquidity) * TEN_THOUSAND * sudt_reserve
                == BigUint::from(sudt_injected) * 9985u128 * total_liquidity
            {
                verify_price(ckb_injected, sudt_injected, liquidity_cell_data.price, 15)?;
            } else {
                return Err(Error::LiquidityPoolTokenDiff);
            }
        } else if change_cell_data.len() >= 16 {
            if get_cell_type_hash(&change_cell)?.unpack()
                != get_cell_type_hash(&pool_out_cell)?.unpack()
                || change_cell.lock().code_hash().as_slice()
                    != liquidity_order_cell.lock().args().as_slice()
            {
                return Err(Error::InvalidChangeCell);
            }

            sudt_injected =
                liquidity_order_data.sudt_amount - decode_u128(&change_cell_data[..16])?;
            ckb_injected = liquidity_order_cell.capacity().unpack() as u128 - SUDT_CAPACITY * 2;

            if BigUint::from(user_liquidity) * TEN_THOUSAND * ckb_reserve
                == BigUint::from(ckb_injected) * 9985u128 * total_liquidity
            {
                verify_price(ckb_injected, sudt_injected, liquidity_cell_data.price, 15)?;
            } else {
                return Err(Error::LiquidityPoolTokenDiff);
            }
        } else {
            return Err(Error::InvalidChangeCell);
        }

        *ckb_collect += ckb_injected;
        *sudt_collect += sudt_injected;
        *user_liquidity_mint += user_liquidity;
    } else {
        if ckb_reserve != 0 || sudt_reserve != 0 || total_liquidity != 0 {
            return Err(Error::MintLiquidityFailed);
        }

        if get_cell_type_hash(&liquidity_cell)?.unpack()[0..20]
            != info_out_data.liquidity_sudt_type_hash
            || liquidity_cell.lock().code_hash().as_slice()
                != liquidity_order_cell.lock().args().as_slice()
        {
            return Err(Error::InvalidLiquidityCell);
        }

        let sudt_injected = liquidity_order_data.sudt_amount;
        let ckb_injected = liquidity_order_cell.capacity().unpack() as u128 - SUDT_CAPACITY;
        let mint_liquidity: u128 = (BigUint::from(sudt_injected) * ckb_injected)
            .sqrt()
            .try_into()
            .unwrap();

        if BigUint::from(user_liquidity) * TEN_THOUSAND != BigUint::from(mint_liquidity) * 9985u128
        {
            return Err(Error::LiquidityPoolTokenDiff);
        }

        *ckb_collect += ckb_injected;
        *sudt_collect += sudt_injected;
        *user_liquidity_mint += user_liquidity;
    }

    Ok(())
}

fn burn_liquidity(
    index: usize,
    liquidity_order_cell: &CellOutput,
    liquidity_order_data: &LiquidityOrderCellData,
    pool_out_cell: &CellOutput,
    ckb_reserve: u128,
    sudt_reserve: u128,
    total_liquidity: u128,
    pool_ckb_paid: &mut u128,
    pool_sudt_paid: &mut u128,
    user_liquidity_burned: &mut u128,
) -> Result<(), Error> {
    if total_liquidity == 0 || liquidity_order_data.sudt_amount == 0 {
        return Err(Error::BurnLiquidityFailed);
    }

    let ckb_sudt_out = load_cell(index, Source::Output)?;
    let ckb_sudt_data = load_cell_data(index, Source::Output)?;

    if ckb_sudt_data.len() < 16
        || get_cell_type_hash(&ckb_sudt_out)?.unpack()
            != get_cell_type_hash(&pool_out_cell)?.unpack()
        || ckb_sudt_out.lock().code_hash().as_slice()
            != liquidity_order_cell.lock().args().as_slice()
    {
        return Err(Error::InvalidLiquidityCell);
    }

    let user_ckb_got =
        (ckb_sudt_out.capacity().unpack() - liquidity_order_cell.capacity().unpack()) as u128;
    let user_sudt_got = decode_u128(&ckb_sudt_data[0..20])?;
    let burned_liquidity = liquidity_order_data.sudt_amount;

    verify_price(user_ckb_got, user_sudt_got, liquidity_order_data.price, 15)?;

    if BigUint::from(total_liquidity) * TEN_THOUSAND * user_sudt_got
        != BigUint::from(burned_liquidity) * 9985u128 * sudt_reserve
    {
        return Err(Error::LiquidityPoolTokenDiff);
    }

    *pool_ckb_paid += user_ckb_got;
    *pool_sudt_paid += user_sudt_got;
    *user_liquidity_burned += burned_liquidity;

    assert!(*pool_ckb_paid < ckb_reserve);
    assert!(*pool_sudt_paid < sudt_reserve);
    Ok(())
}

fn verify_price(amount_0: u128, amount_1: u128, price: Price, slipage: u128) -> Result<(), Error> {
    if price.exponent < 0 {
        let exp = price.exponent.abs() as u32;
        let price = BigUint::from(price.coefficient) * BigUint::from(10u8).pow(exp);
        let amount_1 = BigUint::from(amount_1);

        if price * amount_0 * TEN_THOUSAND > amount_1 * (TEN_THOUSAND + slipage)
            || price * amount_0 * TEN_THOUSAND < amount_1 * (TEN_THOUSAND - slipage)
        {
            return Err(Error::VerifyPriceFailed);
        }
    } else {
        let price =
            BigUint::from(price.coefficient) * BigUint::from(10u8).pow(price.exponent as u32);
        let amount_0 = BigUint::from(amount_0);

        if amount_0 * TEN_THOUSAND > price * amount_1 * (TEN_THOUSAND + slipage)
            || amount_0 * TEN_THOUSAND < price * amount_1 * (TEN_THOUSAND - slipage)
        {
            return Err(Error::VerifyPriceFailed);
        }
    }
    Ok(())
}
