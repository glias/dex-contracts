// Order book script
//
// An order book script using 41bytes cell data
//
// This order book script has three scenarios:
//
// 1. The placing order operation will generate cells, which contain sudt type script and
// data conforming to certain rules. Cell data includes four fields: sudt_ mount(uint128),
// order amount(uint128), price(uint64), order type(uint8).
//
// 2. When the prices and quantities of different buy and sell orders match, they will be
// matched into a transaction to complete the purchase needs of both buyers and sellers.
// At the same time, the cell data fields of inputs and outputs will be updated accordingly.
//
// 3. Order cancellation and withdrawal operations require additional cells to be placed in inputs,
// and the signature verification of the order book cell is achieved by verifying the signature of
// the additional cell. At the same time, the order book lock args must be equal to the lock hash
// of the additional cell.
//

use core::result::Result;

use share::ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    default_alloc,
    high_level::{load_cell, load_script, load_witness_args, QueryIter},
};

use share::error::Error;
use share::hash::blake2b_256;

mod order;

// Alloc 4K fast HEAP + 2M HEAP to receives PrefilledData
default_alloc!(4 * 1024, 2048 * 1024, 64);

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();

    // The length of args(lock hash) must be 32 bytes
    if args.len() != 32 {
        return Err(Error::InvalidArgument);
    }

    // Check if there is an input with lock hash equal to order book cell lock args in Inputs.
    // If it exists, verify it according to the process of withdrawal or withdrawal,
    // if it does not exist, verify it according to the process of matching transaction.
    let input_position = QueryIter::new(load_cell, Source::Input)
        .position(|cell| &blake2b_256(cell.lock().as_slice())[..] == &args[..]);

    match input_position {
        None => return order::validate(),
        // If it is an order cancellation or withdrawal operation, inputs must contain an input
        // whose witness is not empty, and the lock hash of this input is equal to order
        // book cell lock args.
        Some(position) => match load_witness_args(position, Source::Input) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::WrongMatchInputWitness),
        },
    }
}
