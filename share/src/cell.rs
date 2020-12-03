use core::cmp::{Eq, PartialEq};
use core::convert::TryFrom;
use core::result::Result;

use crate::error::Error;
use crate::{check_args_len, decode_i8, decode_u128, decode_u64, decode_u8};

use ckb_std::ckb_types::bytes::Bytes;

const LIQUIDITY_ORDER_DATA_LEN: usize = 46;
const ASSET_ORDER_DATA_LEN: usize = 43;
const INFO_CELL_DATA_LEN: usize = 68;
const POOL_CELL_DATA_LEN: usize = 16;
const SUDT_CELL_DATA_LEN: usize = 16;

#[derive(Debug, PartialEq, Eq)]
pub enum OrderKind {
    Sell,
    Buy,
}

impl TryFrom<u8> for OrderKind {
    type Error = Error;

    fn try_from(input: u8) -> Result<OrderKind, Error> {
        match input {
            0 => Ok(OrderKind::Sell),
            1 => Ok(OrderKind::Buy),
            _ => Err(Error::InvalidOrderKind),
        }
    }
}

impl Into<u8> for OrderKind {
    fn into(self) -> u8 {
        match self {
            OrderKind::Sell => 0,
            OrderKind::Buy => 1,
        }
    }
}

#[derive(Debug)]
pub struct LiquidityOrderCellData {
    pub sudt_amount:    u128,
    pub version:        u8,
    pub price:          u64,
    pub exponent:       i8,
    pub info_type_hash: Bytes,
}

impl LiquidityOrderCellData {
    pub fn from_raw(cell_raw_data: &[u8]) -> Result<LiquidityOrderCellData, Error> {
        check_args_len(cell_raw_data.len(), LIQUIDITY_ORDER_DATA_LEN)?;

        let sudt_amount = decode_u128(&cell_raw_data[..16])?;
        let version = decode_u8(&cell_raw_data[16..17])?;
        let price = decode_u64(&cell_raw_data[17..25])?;
        let exponent = decode_i8(&cell_raw_data[25..26])?;
        let mut buf = [0u8; 20];
        buf.copy_from_slice(&cell_raw_data[26..46]);
        let info_type_hash = Bytes::from(buf.to_vec());

        let liquidity_order = LiquidityOrderCellData {
            sudt_amount,
            version,
            price,
            exponent,
            info_type_hash,
        };

        Ok(liquidity_order)
    }
}

#[derive(Debug)]
pub struct AssetOrderCellData {
    pub sudt_amount:  u128,
    pub order_amount: u128,
    pub price:        u64,
    pub exponent:     i8,
    pub kind:         OrderKind,
    pub version:      u8,
}

impl AssetOrderCellData {
    pub fn from_raw(cell_raw_data: &[u8]) -> Result<AssetOrderCellData, Error> {
        check_args_len(cell_raw_data.len(), ASSET_ORDER_DATA_LEN)?;

        let sudt_amount = decode_u128(&cell_raw_data[..16])?;
        let order_amount = decode_u128(&cell_raw_data[16..32])?;
        let price = decode_u64(&cell_raw_data[32..40])?;
        let exponent = decode_i8(&cell_raw_data[40..41])?;
        let kind = OrderKind::try_from(decode_u8(&cell_raw_data[41..42])?)?;
        let version = decode_u8(&cell_raw_data[42..43])?;

        Ok(AssetOrderCellData {
            sudt_amount,
            order_amount,
            price,
            exponent,
            kind,
            version,
        })
    }
}

#[derive(Debug)]
pub struct InfoCellData {
    pub ckb_reserve:              u128,
    pub token_reserve:            u128,
    pub total_liquidity:          u128,
    pub liquidity_sudt_type_hash: [u8; 20],
}

impl InfoCellData {
    pub fn from_raw(cell_raw_data: &[u8]) -> Result<InfoCellData, Error> {
        check_args_len(cell_raw_data.len(), INFO_CELL_DATA_LEN)?;

        let ckb_reserve = decode_u128(&cell_raw_data[..16])?;
        let token_reserve = decode_u128(&cell_raw_data[16..32])?;
        let total_liquidity = decode_u128(&cell_raw_data[32..48])?;
        let mut liquidity_sudt_type_hash = [0u8; 20];
        liquidity_sudt_type_hash.copy_from_slice(&cell_raw_data[48..68]);

        Ok(InfoCellData {
            ckb_reserve,
            token_reserve,
            total_liquidity,
            liquidity_sudt_type_hash,
        })
    }
}

#[derive(Debug)]
pub struct PoolCellData {
    pub sudt_amount: u128,
}

impl PoolCellData {
    pub fn from_raw(cell_raw_data: &[u8]) -> Result<PoolCellData, Error> {
        check_args_len(cell_raw_data.len(), POOL_CELL_DATA_LEN)?;
        let sudt_amount = decode_u128(&cell_raw_data[..16])?;

        Ok(PoolCellData { sudt_amount })
    }
}

#[derive(Debug)]
pub struct SUDTCellData {
    pub amount: u128,
}

impl SUDTCellData {
    pub fn from_raw(cell_raw_data: &[u8]) -> Result<SUDTCellData, Error> {
        check_args_len(cell_raw_data.len(), SUDT_CELL_DATA_LEN)?;
        let amount = decode_u128(&cell_raw_data[..16])?;

        Ok(SUDTCellData { amount })
    }
}
