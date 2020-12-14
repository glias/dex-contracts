use alloc::vec::Vec;
use core::convert::TryFrom;
use core::result::Result;

use ckb_std::ckb_types::packed::Script;
use ckb_std::error::SysError;
use ckb_std::high_level::{
    load_cell_capacity, load_cell_data, load_cell_lock, load_cell_lock_hash, load_cell_type_hash,
    load_input, QueryIter,
};
use ckb_std::{ckb_constants::Source, ckb_types::prelude::*};
use num_bigint::BigUint;

use crate::error::Error;

// The dex fee rate is fixed at 0.3%
const FEE: u128 = 3;
const FEE_DECIMAL: u128 = 1000;

// The cell data length of order book is fixed at 41 bytes
const ORDER_DATA_LEN: usize = 43;
const PRICE_BYTES_LEN: usize = 9;

#[derive(Debug)]
struct Price {
    effect:   u64,
    exponent: i8,
}

impl TryFrom<[u8; PRICE_BYTES_LEN]> for Price {
    type Error = Error;

    fn try_from(bytes: [u8; PRICE_BYTES_LEN]) -> Result<Price, Self::Error> {
        let effect = {
            let mut buf = [0u8; 8];
            buf.copy_from_slice(&bytes[0..8]);
            u64::from_le_bytes(buf)
        };
        if effect == 0 {
            return Err(Error::PriceIsZero);
        }

        let exponent = {
            let mut buf = [0u8; 1];
            buf.copy_from_slice(&bytes[8..9]);
            i8::from_le_bytes(buf)
        };
        if exponent < -100 || exponent > 100 {
            return Err(Error::PriceExponentOutOfRange);
        }

        Ok(Price { effect, exponent })
    }
}

impl Price {
    fn is_exponent_negative(&self) -> bool {
        self.exponent < 0
    }

    fn biguint_exponent(&self) -> BigUint {
        let exp = u32::from(self.exponent.abs() as u32);
        assert!(exp <= 100, "exponent range is -100 to 100");
        BigUint::from(10u8).pow(exp)
    }

    fn biguint_effect(&self) -> BigUint {
        BigUint::from(self.effect)
    }
}

impl PartialEq for Price {
    fn eq(&self, other: &Self) -> bool {
        self.effect == other.effect && self.exponent == other.exponent
    }
}

impl Eq for Price {}

#[derive(Debug, PartialEq, Eq)]
enum OrderType {
    SellCKB = 0,
    BuyCKB = 1,
}

impl TryFrom<u8> for OrderType {
    type Error = Error;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0 => Ok(OrderType::SellCKB),
            1 => Ok(OrderType::BuyCKB),
            _ => Err(Error::UnknownOrderType),
        }
    }
}

#[derive(Debug)]
struct Order {
    sudt_amount:  u128,
    version:      u8,
    order_amount: u128,
    price:        Price,
    type_:        OrderType,
}

impl TryFrom<&[u8]> for Order {
    type Error = Error;

    fn try_from(cell_data: &[u8]) -> Result<Order, Self::Error> {
        if cell_data.len() != ORDER_DATA_LEN {
            return Err(Error::WrongDataLengthOrFormat);
        }

        let mut data_buf = [0u8; ORDER_DATA_LEN];
        data_buf.copy_from_slice(&cell_data);

        let mut sudt_amount_buf = [0u8; 16];
        let mut version_buf = [0u8; 1];
        let mut order_amount_buf = [0u8; 16];
        let mut price_buf = [0u8; PRICE_BYTES_LEN];
        let mut order_type_buf = [0u8; 1];

        sudt_amount_buf.copy_from_slice(&data_buf[0..16]);
        version_buf.copy_from_slice(&data_buf[16..17]);
        order_amount_buf.copy_from_slice(&data_buf[17..33]);
        price_buf.copy_from_slice(&data_buf[33..42]);
        order_type_buf.copy_from_slice(&data_buf[42..43]);

        let order = Order {
            sudt_amount:  u128::from_le_bytes(sudt_amount_buf),
            version:      u8::from_le_bytes(version_buf),
            order_amount: u128::from_le_bytes(order_amount_buf),
            price:        Price::try_from(price_buf)?,
            type_:        OrderType::try_from(u8::from_le_bytes(order_type_buf))?,
        };

        if order.version != 1 {
            return Err(Error::UnexpectedVersion);
        }

        Ok(order)
    }
}

#[derive(Debug)]
struct Cell {
    index:  usize,
    source: Source,

    capacity:    u64,
    data:        Vec<u8>,
    lock_script: Script,
    lock_hash:   [u8; 32],
}

impl Cell {
    pub fn load(index: usize, source: Source) -> Result<Self, SysError> {
        let cell = Cell {
            index,
            source,

            capacity: load_cell_capacity(index, source)?,
            data: load_cell_data(index, source)?,
            lock_script: load_cell_lock(index, source)?,
            lock_hash: load_cell_lock_hash(index, source)?,
        };

        Ok(cell)
    }

    pub fn type_hash(&self) -> Result<Option<[u8; 32]>, SysError> {
        load_cell_type_hash(self.index, self.source)
    }

    pub fn to_order(&self) -> Result<Order, Error> {
        Order::try_from(self.data.as_slice())
    }

    pub fn sudt_amount(&self) -> Result<u128, Error> {
        if self.data.len() < 16 {
            return Err(Error::WrongDataLengthOrFormat);
        }

        let mut buf = [0u8; 16];
        buf.copy_from_slice(&self.data.as_slice()[0..16]);
        Ok(u128::from_le_bytes(buf))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum OrderState {
    PartialFilled,
    Completed,
}

fn validate_sell_ckb_order(input: &Cell, output: &Cell, state: OrderState) -> Result<(), Error> {
    if state == OrderState::Completed {
        if output.type_hash() != input.type_hash() {
            return Err(Error::TypeHashChanged);
        }

        if output.data.len() < 16 {
            return Err(Error::NotASudtCell);
        }
    }

    if output.capacity > input.capacity {
        return Err(Error::NegativeCapacityDifference);
    }

    let input_sudt_amount = input.sudt_amount()?;
    let output_sudt_amount = output.sudt_amount()?;
    if output_sudt_amount == 0 || output_sudt_amount < input_sudt_amount {
        return Err(Error::NegativeSudtDifference);
    }

    let ckb_sold = input.capacity - output.capacity;
    let sudt_got = output_sudt_amount - input_sudt_amount;

    let order = input.to_order()?;
    let price_exponent = order.price.biguint_exponent();
    let price_effect = order.price.biguint_effect();
    if order.price.is_exponent_negative() {
        if BigUint::from(FEE_DECIMAL - FEE) * ckb_sold * price_exponent.clone()
            > BigUint::from(FEE_DECIMAL) * sudt_got * price_effect.clone()
        {
            return Err(Error::PriceNotMatched);
        }
    } else {
        let price = price_exponent.clone() * price_effect.clone();

        if BigUint::from(FEE_DECIMAL - FEE) * ckb_sold
            > BigUint::from(FEE_DECIMAL) * sudt_got * price
        {
            return Err(Error::PriceNotMatched);
        }
    }

    if state == OrderState::Completed {
        // Only allow 99% filled order which cannot sell more ckb with this price to
        // complete
        let remained = BigUint::from(order.order_amount - sudt_got);
        if order.price.is_exponent_negative() {
            if remained * price_exponent > price_effect {
                return Err(Error::UnmatchableOrder);
            }
        } else if remained > price_exponent * price_effect {
            return Err(Error::UnmatchableOrder);
        }
    }

    Ok(())
}

fn validate_buy_ckb_order(input: &Cell, output: &Cell, state: OrderState) -> Result<(), Error> {
    if state == OrderState::Completed {
        match output.type_hash()? {
            // Allow 99% filled order to complete, if it cannot buy more ckb with given
            // order price. We should have a sudt cell here.
            Some(_sudt_type) if output.data.len() < 16 => return Err(Error::NotASudtCell),
            None if output.data.len() != 0 => return Err(Error::NotAFreeCell),
            _ => (),
        }
    }

    if input.capacity > output.capacity {
        return Err(Error::NegativeCapacityDifference);
    }

    let input_sudt_amount = input.sudt_amount()?;
    if input_sudt_amount == 0 {
        return Err(Error::InputSudtIsZero);
    }

    let output_sudt_amount = output.sudt_amount().unwrap_or_else(|_| 0);
    if output_sudt_amount > input_sudt_amount {
        return Err(Error::NegativeSudtDifference);
    }

    let ckb_bought: u64 = output.capacity - input.capacity;
    let sudt_paid = input_sudt_amount - output_sudt_amount;

    let order = input.to_order()?;
    let price_exponent = order.price.biguint_exponent();
    let price_effect = order.price.biguint_effect();
    if order.price.is_exponent_negative() {
        if BigUint::from(FEE_DECIMAL) * ckb_bought * price_exponent.clone()
            < BigUint::from(FEE_DECIMAL - FEE) * sudt_paid * price_effect.clone()
        {
            return Err(Error::PriceNotMatched);
        }
    } else {
        let price = price_exponent.clone() * price_effect.clone();

        if BigUint::from(FEE_DECIMAL) * ckb_bought
            < BigUint::from(FEE_DECIMAL - FEE) * price * sudt_paid
        {
            return Err(Error::PriceNotMatched);
        }
    }

    if state == OrderState::Completed {
        // Only allow partial filled order which cannot buy more ckb with this price to
        // complete
        let remained = BigUint::from(order.order_amount - u128::from(ckb_bought));
        if order.price.is_exponent_negative() {
            if remained * price_exponent > price_effect {
                return Err(Error::UnmatchableOrder);
            }
        } else if remained > price_exponent * price_effect {
            return Err(Error::UnmatchableOrder);
        }
    }

    Ok(())
}

fn validate_order_cells(index: usize) -> Result<(), Error> {
    // TODO: cancel order
    let input = Cell::load(index, Source::Input)?;
    let output = Cell::load(index, Source::Output)?;

    debug!("input {:?}", input);
    debug!("output {:?}", output);

    let input_order = input.to_order()?;
    debug!("input order {:?}", input_order);

    if input_order.order_amount == 0 {
        return Err(Error::OrderAmountIsZero);
    }

    if output.lock_hash != input.lock_hash
        && output.lock_hash != input.lock_script.args().raw_data().as_ref()
    {
        return Err(Error::UnknownLock);
    }

    let state = if output.lock_hash == input.lock_hash {
        if output.type_hash() != input.type_hash() {
            return Err(Error::TypeHashChanged);
        }

        if output.data.len() != input.data.len() {
            return Err(Error::WrongDataLengthOrFormat);
        }

        let output_order = output.to_order()?;
        if output_order.price != input_order.price {
            return Err(Error::PriceChanged);
        }

        if output_order.type_ != input_order.type_ {
            return Err(Error::OrderTypeChanged);
        }

        if output_order.version != input_order.version {
            return Err(Error::UnexpectedVersion);
        }

        if output_order.order_amount == 0 {
            return Err(Error::OrderAmountIsZero);
        }

        OrderState::PartialFilled
    } else {
        OrderState::Completed
    };

    match input_order.type_ {
        OrderType::SellCKB => validate_sell_ckb_order(&input, &output, state),
        OrderType::BuyCKB => validate_buy_ckb_order(&input, &output, state),
    }
}

pub fn validate() -> Result<(), Error> {
    // Find inputs in current group
    let order_inputs = QueryIter::new(load_input, Source::GroupInput).collect::<Vec<_>>();
    // Find all inputs in the current transaction
    let inputs = QueryIter::new(load_input, Source::Input).collect::<Vec<_>>();

    // Find the position of the order book input in the entire inputs to find the output
    // corresponding to the position, and then verify the order data of the input and output
    for index in 0..inputs.len() {
        debug!("index {}", index);

        let input = inputs.get(index).unwrap();
        if order_inputs
            .iter()
            .any(|order_input| order_input.as_slice() == input.as_slice())
        {
            match validate_order_cells(index) {
                Ok(_) => continue,
                Err(err) => return Err(err),
            }
        }
    }

    Ok(())
}
