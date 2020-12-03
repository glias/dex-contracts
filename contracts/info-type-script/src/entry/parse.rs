use core::convert::TryFrom;

// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
// use share::ckb_std::debug;
use share::error::Error;
use share::{check_args_len, decode_u128, decode_u8};

use crate::entry::liquidity::{
    AssetOrderCellData, InfoCellData, LiquidityOrderCellData, OrderKind, PoolCellData, SUDTCellData,
};

const LIQUIDITY_ORDER_DATA_LEN: usize = 65;
const LIQUIDITY_ORDER_LOCK_ARGS_LEN: usize = 53;
const ASSET_ORDER_DATA_LEN: usize = 49;
const INFO_CELL_DATA_LEN: usize = 48;
const POOL_CELL_DATA_LEN: usize = 16;
const SUDT_CELL_DATA_LEN: usize = 16;

const LIQUIDITY_ORDER_CODE_HASH: [u8; 32] = [0u8; 32];
const ASSET_ORDER_CODE_HASH: [u8; 32] = [1u8; 32];

pub fn parse_liquidity_order_cell_data(
    cell_raw_data: &[u8],
) -> Result<LiquidityOrderCellData, Error> {
    check_args_len(cell_raw_data.len(), LIQUIDITY_ORDER_DATA_LEN)?;

    let token_amount = decode_u128(&cell_raw_data[..16])?;
    let maker_fee = decode_u128(&cell_raw_data[16..32])?;
    let price_ckb_amount = decode_u128(&cell_raw_data[32..48])?;
    let price_token_amount = decode_u128(&cell_raw_data[48..64])?;
    let kind = OrderKind::try_from(decode_u8(&cell_raw_data[64..])?)?;

    let liquidity_order = LiquidityOrderCellData {
        token_amount,
        maker_fee,
        price_ckb_amount,
        price_token_amount,
        kind,
    };

    Ok(liquidity_order)
}

pub fn parse_asset_order_cell_data(cell_raw_data: &[u8]) -> Result<AssetOrderCellData, Error> {
    check_args_len(cell_raw_data.len(), ASSET_ORDER_DATA_LEN)?;

    let token_amount = decode_u128(&cell_raw_data[..16])?;
    let order_amount = decode_u128(&cell_raw_data[16..32])?;
    let price = decode_u128(&cell_raw_data[32..48])?;
    let kind = OrderKind::try_from(decode_u8(&cell_raw_data[48..])?)?;

    Ok(AssetOrderCellData {
        token_amount,
        order_amount,
        price,
        kind,
    })
}

pub fn parse_info_cell_data(cell_raw_data: &[u8]) -> Result<InfoCellData, Error> {
    check_args_len(cell_raw_data.len(), INFO_CELL_DATA_LEN)?;

    let ckb_reserve = decode_u128(&cell_raw_data[..16])?;
    let token_reserve = decode_u128(&cell_raw_data[16..32])?;
    let total_liquidity = decode_u128(&cell_raw_data[32..48])?;

    Ok(InfoCellData {
        ckb_reserve,
        token_reserve,
        total_liquidity,
    })
}

pub fn parse_pool_cell_data(cell_raw_data: &[u8]) -> Result<PoolCellData, Error> {
    check_args_len(cell_raw_data.len(), POOL_CELL_DATA_LEN)?;
    let token_amount = decode_u128(&cell_raw_data[..16])?;

    Ok(PoolCellData { token_amount })
}

pub fn parse_sudt_cell_data(cell_raw_data: &[u8]) -> Result<SUDTCellData, Error> {
    check_args_len(cell_raw_data.len(), SUDT_CELL_DATA_LEN)?;
    let amount = decode_u128(&cell_raw_data[..16])?;

    Ok(SUDTCellData { amount })
}
