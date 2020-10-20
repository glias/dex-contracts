// Import from `core` instead of from `std` since we are in no-std mode
use core::result::Result;

// Import heap related library from `alloc`
// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
use ckb_std::{
  ckb_constants::Source,
  ckb_types::prelude::*,
  error::SysError,
  high_level::{
    load_cell_capacity, load_cell_data, load_transaction, load_input, load_cell_type_hash
  },
};

use share::error::Error;

const FEE: f64 = 0.003;
const ORDER_LEN: usize = 57;
const SUDT_LEN: usize = 16;
// real price * 10 ^ 10 = cell price data
const PRICE_PARAM: f64 = 10000000000.0;
const PRECISION_NUMBER: f64 = 0.0001;

struct OrderData {
  sudt_amount: u128,
  dealt_amount: u128,
  undealt_amount: u128,
  price: u64,
  order_type: u8,
}

fn _init_order_data() -> OrderData {
  OrderData {
    sudt_amount: 0u128,
    dealt_amount: 0u128,
    undealt_amount: 0u128,
    price: 0u64,
    order_type: 0u8,
  }
}


fn parse_order_data(data: &[u8]) -> Result<OrderData, Error> {
  // sudt_amount(u128) or sudt_amount(u128) + dealt(u128) + undealt(u128) + price(u64) + order_type(u8)
  if data.len() != SUDT_LEN && data.len() != ORDER_LEN {
    return Err(Error::WrongDataLengthOrFormat);
  }
  let mut sudt_amount_buf = [0u8; 16];
  let mut dealt_amount_buf = [0u8; 16];
  let mut undealt_amount_buf = [0u8; 16];
  let mut price_buf = [0u8; 8];
  let mut order_type_buf = [0u8; 1];

  sudt_amount_buf.copy_from_slice(&data[0..16]);
  if data.len() == ORDER_LEN {
    dealt_amount_buf.copy_from_slice(&data[16..32]);
    undealt_amount_buf.copy_from_slice(&data[32..48]);
    price_buf.copy_from_slice(&data[48..56]);
    order_type_buf.copy_from_slice(&data[56..57]);
  }
  Ok(OrderData {
    sudt_amount: u128::from_le_bytes(sudt_amount_buf),
    dealt_amount: u128::from_le_bytes(dealt_amount_buf),
    undealt_amount: u128::from_le_bytes(undealt_amount_buf),
    price: u64::from_le_bytes(price_buf),
    order_type: u8::from_le_bytes(order_type_buf),
  })
}

fn parse_cell_data(index: usize, source: Source) -> Result<OrderData, Error> {
  let data = match load_cell_data(index, source) {
      Ok(data) => data,
      Err(SysError::IndexOutOfBound) => return Err(Error::IndexOutOfBound),
      Err(err) => return Err(err.into()),
  };
  return match data.len() {
    ORDER_LEN => {
      let mut data_buf = [0u8; ORDER_LEN];
      data_buf.copy_from_slice(&data);
      Ok(parse_order_data(&data_buf)?)
    }
    SUDT_LEN => {
      let mut data_buf = [0u8; SUDT_LEN];
      data_buf.copy_from_slice(&data);
      Ok(parse_order_data(&data_buf)?)
    }
    _ => Err(Error::WrongDataLengthOrFormat),
  };
}

fn validate_order_cells(index: usize) -> Result<(), Error> {
  let input_type_hash = match load_cell_type_hash(index, Source::Input) {
    Ok(hash) => hash,
    Err(err) => return Err(err.into())
  };
  let output_type_hash = match load_cell_type_hash(index, Source::Output) {
    Ok(hash) => hash,
    Err(err) => return Err(err.into())
  };
  if input_type_hash != output_type_hash {
    return Err(Error::TypeHashNotSame);
  }
  let input_capacity = load_cell_capacity(index, Source::Input)?;
  let output_capacity = load_cell_capacity(index, Source::Output)?;
  let input_order = parse_cell_data(index, Source::Input)?;
  let output_order = parse_cell_data(index, Source::Output)?;

  if input_order.undealt_amount == 0 {
    return Err(Error::WrongSUDTInputAmount);
  }
  if input_order.price == 0 {
    return Err(Error::OrderPriceNotZero);
  }

  if output_order.dealt_amount != 0 {
    if input_order.order_type != output_order.order_type {
      return Err(Error::WrongOrderType);
    }

    if input_order.dealt_amount > output_order.dealt_amount {
      return Err(Error::WrongSUDTDiffAmount);
    }
  }

  let order_price: f64 = input_order.price as f64 / PRICE_PARAM;
 
  // Buy SUDT
  if input_order.order_type == 0 {
    if input_capacity < output_capacity {
      return Err(Error::WrongDiffCapacity);
    }
    if input_order.sudt_amount > output_order.sudt_amount || input_order.undealt_amount < output_order.undealt_amount  {
      return Err(Error::WrongSUDTDiffAmount);
    }

    let diff_undealt_amount = (input_order.undealt_amount - output_order.undealt_amount) as f64;

    if output_order.dealt_amount != 0 {
      let diff_dealt_amount = (output_order.dealt_amount - input_order.dealt_amount) as f64;

      if diff_dealt_amount != diff_undealt_amount {
        return Err(Error::WrongSUDTDiffAmount);
      }
    }

    let diff_capacity = (input_capacity - output_capacity) as f64;
    let diff_sudt_amount = (output_order.sudt_amount - input_order.sudt_amount) as f64;
    
    if diff_sudt_amount != diff_undealt_amount {
      return Err(Error::WrongSUDTDiffAmount);
    }

    if diff_undealt_amount * (1.0 + FEE) * order_price + PRECISION_NUMBER < diff_capacity {
      return Err(Error::WrongSwapAmount);
    }
  } else if input_order.order_type == 1 {
    // Sell SUDT
    if input_capacity > output_capacity {
      return Err(Error::WrongDiffCapacity);
    }

    if input_order.sudt_amount < output_order.sudt_amount || input_order.undealt_amount < output_order.undealt_amount {
      return Err(Error::WrongSUDTDiffAmount);
    }

    let diff_undealt_amount = (input_order.undealt_amount - output_order.undealt_amount) as f64;

    if output_order.dealt_amount != 0 {
      let diff_dealt_amount = (output_order.dealt_amount - input_order.dealt_amount) as f64;

      if diff_dealt_amount != diff_undealt_amount {
        return Err(Error::WrongSUDTDiffAmount);
      }
    }

    let diff_capacity = (output_capacity - input_capacity) as f64;
    let diff_sudt_amount = (input_order.sudt_amount - output_order.sudt_amount) as f64;
    
    // Floating point numbers have precision errors
    if diff_sudt_amount - diff_undealt_amount * (1.0 + FEE) > PRECISION_NUMBER {
      return Err(Error::WrongSUDTDiffAmount);
    }

    if diff_capacity * (1.0 + FEE) + PRECISION_NUMBER < diff_sudt_amount * order_price {
      return Err(Error::WrongSwapAmount);
    }
  } else {
    return Err(Error::WrongOrderType);
  }

  Ok(())
}


pub fn validate() -> Result<(), Error> {
  let tx = match load_transaction() {
    Ok(tx) => tx.raw(),
    Err(err) => return Err(err.into()),
  };

  let inputs_count = tx.inputs().len();
  if inputs_count != tx.outputs().len() {
    return Err(Error::InputsAndOutputsAmountNotSame);
  }

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
