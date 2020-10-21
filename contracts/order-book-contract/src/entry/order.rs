// Import from `core` instead of from `std` since we are in no-std mode
use core::result::Result;

// Import heap related library from `alloc`
// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
use ckb_std::{
  ckb_constants::Source,
  ckb_types::prelude::*,
  high_level::{
    load_cell_capacity, load_cell_data, load_transaction, load_input, load_cell_type_hash
  },
};

use share::error::Error;

const FEE: u128 = 3;  // 0.3%
const ORDER_LEN: usize = 41;
const FEE_DECIMAL: u128 = 1000;
const PRICE_DECIMAL: u128 = 10000000000;  // order_price = real_price * 10^10

struct OrderData {
  sudt_amount: u128,
  order_amount: u128,
  price: u64,
  order_type: u8,
}

fn parse_order_data(data: &[u8]) -> Result<OrderData, Error> {
  // sudt_amount(u128) + order_amount(u128) + price(u64) + order_type(u8)
  let mut sudt_amount_buf = [0u8; 16];
  let mut order_amount_buf = [0u8; 16];
  let mut price_buf = [0u8; 8];
  let mut order_type_buf = [0u8; 1];

  sudt_amount_buf.copy_from_slice(&data[0..16]);
  order_amount_buf.copy_from_slice(&data[16..32]);
  price_buf.copy_from_slice(&data[32..40]);
  order_type_buf.copy_from_slice(&data[40..41]);

  Ok(OrderData {
    sudt_amount: u128::from_le_bytes(sudt_amount_buf),
    order_amount: u128::from_le_bytes(order_amount_buf),
    price: u64::from_le_bytes(price_buf),
    order_type: u8::from_le_bytes(order_type_buf),
  })
}

fn parse_cell_data(index: usize, source: Source) -> Result<OrderData, Error> {
  let data = load_cell_data(index, source)?;
  return match data.len() {
    ORDER_LEN => {
      let mut data_buf = [0u8; ORDER_LEN];
      data_buf.copy_from_slice(&data);
      Ok(parse_order_data(&data_buf)?)
    }
    _ => Err(Error::WrongDataLengthOrFormat),
  };
}

fn validate_order_cells(index: usize) -> Result<(), Error> {
  let input_type_hash = load_cell_type_hash(index, Source::Input)?;
  let output_type_hash = load_cell_type_hash(index, Source::Output)?;
  if input_type_hash != output_type_hash {
    return Err(Error::TypeHashNotSame);
  }
  let input_capacity = load_cell_capacity(index, Source::Input)?;
  let output_capacity = load_cell_capacity(index, Source::Output)?;
  let input_order = parse_cell_data(index, Source::Input)?;
  let output_order = parse_cell_data(index, Source::Output)?;

  if input_order.order_amount == 0 {
    return Err(Error::WrongSUDTInputAmount);
  }
  if input_order.order_amount < output_order.order_amount {
    return Err(Error::WrongSUDTDiffAmount);
  }
  if input_order.price == 0 || output_order.price == 0 {
    return Err(Error::OrderPriceNotZero);
  }
  if input_order.price != output_order.price {
    return Err(Error::OrderPriceNotSame);
  }
  if input_order.order_type != output_order.order_type {
    return Err(Error::WrongOrderType);
  }

  // Buy SUDT
  if input_order.order_type == 0 {
    if input_capacity < output_capacity {
      return Err(Error::WrongDiffCapacity);
    }
    if input_order.sudt_amount > output_order.sudt_amount {
      return Err(Error::WrongSUDTDiffAmount);
    }

    let diff_sudt_amount = output_order.sudt_amount - input_order.sudt_amount;
    let diff_order_amount = input_order.order_amount - output_order.order_amount;
    let diff_capacity = (input_capacity - output_capacity) as u128;
    
    if diff_sudt_amount != diff_order_amount {
      return Err(Error::WrongSUDTDiffAmount);
    }
    
    let diff_capacity_decimal = diff_capacity * FEE_DECIMAL * PRICE_DECIMAL;
    let diff_sudt_decimal = diff_sudt_amount * (1000 + FEE) * (input_order.price as u128);

    if diff_capacity_decimal > diff_sudt_decimal {
      return Err(Error::WrongSwapAmount);
    }
  } else if input_order.order_type == 1 {
    // Sell SUDT
    if input_capacity > output_capacity {
      return Err(Error::WrongDiffCapacity);
    }

    if input_order.sudt_amount < output_order.sudt_amount {
      return Err(Error::WrongSUDTDiffAmount);
    }

    let diff_sudt_amount = input_order.sudt_amount - output_order.sudt_amount;
    let diff_order_amount = input_order.order_amount - output_order.order_amount;
    let diff_capacity = (output_capacity - input_capacity) as u128;
    
    if diff_capacity != diff_order_amount {
      return Err(Error::WrongDiffCapacity);
    }

    let diff_capacity_decimal = diff_capacity * (1000 + FEE) * PRICE_DECIMAL;
    let diff_sudt_decimal = diff_sudt_amount * (input_order.price as u128) * FEE_DECIMAL; 

    if diff_capacity_decimal < diff_sudt_decimal {
      return Err(Error::WrongSwapAmount);
    }
  } else {
    return Err(Error::WrongOrderType);
  }

  Ok(())
}


pub fn validate() -> Result<(), Error> {
  let inputs_count = load_transaction()?.raw().inputs().len();

  for group_index in 0..inputs_count {
    match load_input(group_index, Source::GroupInput) {
      Ok(group_input) => {
        for index in 0..inputs_count {
          let input = load_input(index, Source::Input).unwrap();
          if group_input.as_slice() == input.as_slice() {
            match validate_order_cells(index) {
              Ok(_) => break,
              Err(err) => return Err(err)
            };
          }
        }
      },
      Err(_) => break,
    };
  }
  
  Ok(())

}
