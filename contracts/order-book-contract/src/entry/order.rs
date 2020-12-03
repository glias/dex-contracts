use alloc::vec::Vec;
use core::result::Result;

use num_bigint::BigUint;
use num_traits::Zero;

use ckb_std::{
    ckb_constants::Source,
    ckb_types::prelude::*,
    debug,
    high_level::{
        load_cell_capacity, load_cell_data, load_cell_lock_hash, load_cell_type_hash, load_input,
        QueryIter,
    },
};

use share::error::Error;
use share::{check_lock_hash, check_type_hash, decode_u128, decode_u64, decode_u8};

// The dex fee rate is fixed at 0.3%
const FEE: u128 = 3;
const FEE_DECIMAL: u128 = 1_000;

// The cell data length of order book is fixed at 41 bytes
const ORDER_LEN: usize = 49;
// order_price = real_price * 10^20
const PRICE_DECIMAL: u128 = 100_000_000_000_000_000_000;

#[derive(Debug)]
// order cell data: sudt_amount(u128) + order_amount(u128) + price(u64) + order_type(u8)
struct OrderData {
    sudt_amount:  u128,
    order_amount: u128,
    price:        BigUint,
    order_type:   u8,
}

fn parse_cell_data(index: usize, source: Source) -> Result<OrderData, Error> {
    let data = load_cell_data(index, source)?;
    return match data.len() {
        ORDER_LEN => {
            let mut data_buf = [0u8; ORDER_LEN];
            data_buf.copy_from_slice(&data);

            let mut sudt_amount_buf = [0u8; 16];
            let mut order_amount_buf = [0u8; 16];
            let mut price_buf = [0u8; 16];
            let mut order_type_buf = [0u8; 1];

            sudt_amount_buf.copy_from_slice(&data_buf[0..16]);
            order_amount_buf.copy_from_slice(&data_buf[16..32]);
            price_buf.copy_from_slice(&data_buf[32..48]);
            order_type_buf.copy_from_slice(&data_buf[48..49]);

            Ok(OrderData {
                sudt_amount:  u128::from_le_bytes(sudt_amount_buf),
                order_amount: u128::from_le_bytes(order_amount_buf),
                price:        BigUint::from_bytes_le(&price_buf),
                order_type:   u8::from_le_bytes(order_type_buf),
            })
        }
        _ => Err(Error::WrongDataLengthOrFormat),
    };
}

fn validate_order_cells(index: usize) -> Result<(), Error> {
    // The lock hash of input and output at the same location should to be same
    let input_lock_hash = load_cell_lock_hash(index, Source::Input)?;
    let output_lock_hash = load_cell_lock_hash(index, Source::Output)?;
    if input_lock_hash != output_lock_hash {
        return Err(Error::LockHashNotSame);
    }

    // The type hash of input and output at the same location should to be same
    let input_type_hash = load_cell_type_hash(index, Source::Input)?;
    let output_type_hash = load_cell_type_hash(index, Source::Output)?;
    if input_type_hash != output_type_hash {
        return Err(Error::TypeHashNotSame);
    }

    let input_capacity = load_cell_capacity(index, Source::Input)?;
    let output_capacity = load_cell_capacity(index, Source::Output)?;
    let input_order = parse_cell_data(index, Source::Input)?;
    let output_order = parse_cell_data(index, Source::Output)?;

    debug!(
        "input capacity {}, output capacity {},",
        input_capacity, output_capacity
    );
    debug!("input order {:?}", input_order);
    debug!("output order {:?}", output_order);

    if input_order.order_amount == 0 {
        return Err(Error::WrongSUDTInputAmount);
    }
    if input_order.order_amount < output_order.order_amount {
        return Err(Error::WrongSUDTDiffAmount);
    }

    let zero = Zero::zero();
    if input_order.price == zero || output_order.price == zero {
        return Err(Error::OrderPriceNotZero);
    }
    if input_order.price != output_order.price {
        return Err(Error::OrderPriceNotSame);
    }
    if input_order.order_type != output_order.order_type {
        return Err(Error::WrongOrderType);
    }

    debug!("order type {}", input_order.order_type);

    // Buy SUDT
    if input_order.order_type == 0 {
        if input_capacity < output_capacity {
            return Err(Error::WrongDiffCapacity);
        }
        if output_order.sudt_amount == 0 {
            return Err(Error::WrongSUDTDiffAmount);
        }
        if input_order.sudt_amount > output_order.sudt_amount {
            return Err(Error::WrongSUDTDiffAmount);
        }

        let diff_sudt_amount = output_order.sudt_amount - input_order.sudt_amount;
        let diff_order_amount = input_order.order_amount - output_order.order_amount;

        // The sudt_amount increase value should to be equal to the order_amount decrease value
        if diff_sudt_amount != diff_order_amount {
            return Err(Error::WrongSUDTDiffAmount);
        }

        let diff_capacity = BigUint::from(input_capacity - output_capacity);
        let diff_capacity_decimal = diff_capacity * FEE_DECIMAL * PRICE_DECIMAL;
        let diff_sudt_decimal = BigUint::from(diff_sudt_amount) * (1000 + FEE) * input_order.price;

        if diff_capacity_decimal > diff_sudt_decimal {
            return Err(Error::WrongSwapAmount);
        }
    } else if input_order.order_type == 1 {
        // Sell SUDT
        if input_capacity > output_capacity {
            return Err(Error::WrongDiffCapacity);
        }

        if input_order.sudt_amount == 0 {
            return Err(Error::WrongSUDTInputAmount);
        }

        if input_order.sudt_amount < output_order.sudt_amount {
            return Err(Error::WrongSUDTDiffAmount);
        }

        let diff_order_amount = input_order.order_amount - output_order.order_amount;
        let diff_capacity = (output_capacity - input_capacity) as u128;
        // The capacity increase value should to be equal to the order_amount decrease value
        if diff_capacity != diff_order_amount {
            return Err(Error::WrongDiffCapacity);
        }

        let diff_capacity_decimal = BigUint::from(diff_capacity) * (1000 + FEE) * PRICE_DECIMAL;
        let diff_sudt_amount = BigUint::from(input_order.sudt_amount - output_order.sudt_amount);
        let diff_sudt_decimal = diff_sudt_amount * input_order.price * FEE_DECIMAL;

        if diff_capacity_decimal < diff_sudt_decimal {
            return Err(Error::WrongSwapAmount);
        }
    } else {
        return Err(Error::WrongOrderType);
    }

    Ok(())
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
