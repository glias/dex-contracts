// Asset order lock script
//
// An Asset order lock script using 43 bytes cell data
//
// This asset order lock script has three scenarios:
//
// 1. The placing order operation will generate cells, which contain sudt type script and
// data conforming to certain rules.
//
// Cell data includes six fields:
// - sudt amount: uint128
// - version: uint8
// - order amount: uint128
// - price effect: uint64
// - price exponent: int8
// - order type: uint8
//
// 2. When the prices and quantities of different buy and sell orders match, they will be
// matched into a transaction to complete the purchase needs of both buyers and sellers.
// At the same time, the cell data fields of inputs and outputs will be updated accordingly.
//
// 3. Order cancellation
//
// There are two ways to cancel an order:
// - Provide witness args and pass built-in supported lock verification. Currently only pw-lock is
//   supported.
// - Provide another input cell, it's lock hash is equal to order's lock args. And that input's
//   witness args must not be empty to be compatible with anyone can pay lock.

use core::result::Result;

use ckb_std::ckb_types::{bytes::Bytes, prelude::*};
use ckb_std::high_level::{load_cell, load_script, load_witness_args, QueryIter};
use ckb_std::{ckb_constants::Source, default_alloc};
use share::hash::blake2b_256;

use crate::error::Error;

// Alloc 4K fast HEAP + 2M HEAP to receives PrefilledData
default_alloc!(4 * 1024, 2048 * 1024, 64);

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();

    // The length of args(lock hash) must be 32 bytes
    if args.len() != 32 {
        return Err(Error::LockArgsNotAHash);
    }

    // Check cancellation
    // First we check whether there's a witness args to cancel directly
    let has_witness = QueryIter::new()

    // Check whether there is an input's lock hash equal to this order lock args.
    // If it exists, verify it according to the process of withdrawal or withdrawal,
    // if it does not exist, verify it according to the process of matching transaction.
    let input_position = QueryIter::new(load_cell, Source::Input)
        .position(|cell| &blake2b_256(cell.lock().as_slice())[..] == &args[..]);

    match input_position {
        None => return crate::order_validator::validate(),
        // If it is an order cancellation or withdrawal operation, inputs must contain an input
        // whose witness is not empty, and the lock hash of this input is equal to order
        // book cell lock args.
        Some(position) => match load_witness_args(position, Source::Input) {
            Ok(_) => Ok(()),
            Err(_) => Err(Error::WrongMatchInputWitness),
        },
    }
}
