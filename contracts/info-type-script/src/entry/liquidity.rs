use core::cmp::{Eq, PartialEq};
use core::convert::TryFrom;
use core::result::Result;

use share::error::Error;

#[derive(PartialEq, Eq)]
pub enum OrderKind {
    SellCkb,
    BuyCkb,
    AddLiquidity,
    RemoveLiquidity,
}

impl TryFrom<u8> for OrderKind {
    type Error = Error;

    fn try_from(input: u8) -> Result<OrderKind, Error> {
        match input {
            0 => Ok(OrderKind::SellCkb),
            1 => Ok(OrderKind::BuyCkb),
            2 => Ok(OrderKind::AddLiquidity),
            3 => Ok(OrderKind::RemoveLiquidity),
            _ => Err(Error::InvalidOrderKind),
        }
    }
}

impl Into<u8> for OrderKind {
    fn into(self) -> u8 {
        match self {
            OrderKind::SellCkb => 0,
            OrderKind::BuyCkb => 1,
            OrderKind::AddLiquidity => 2,
            OrderKind::RemoveLiquidity => 3,
        }
    }
}

impl OrderKind {
    pub fn is_liquidity_opt(&self) -> bool {
        match *self {
            OrderKind::AddLiquidity => true,
            OrderKind::RemoveLiquidity => true,
            _ => false,
        }
    }
}

pub struct LiquidityOrderCellData {
    pub token_amount:       u128,
    pub maker_fee:          u128,
    pub price_ckb_amount:   u128,
    pub price_token_amount: u128,
    pub kind:               OrderKind,
}

pub struct AssetOrderCellData {
    pub token_amount: u128,
    pub order_amount: u128,
    pub price:        u128,
    pub kind:         OrderKind,
}

pub struct InfoCellData {
    pub ckb_reserve:     u128,
    pub token_reserve:   u128,
    pub total_liquidity: u128,
}

pub struct PoolCellData {
    pub token_amount: u128,
}

pub struct SUDTCellData {
    pub amount: u128,
}
