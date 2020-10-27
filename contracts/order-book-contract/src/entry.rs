use core::result::Result;

use ckb_std::{
  default_alloc,
  ckb_constants::Source,
  ckb_types::{bytes::Bytes, prelude::*},
  high_level::{load_script, load_cell, load_witness_args, QueryIter},
};

use share::error::Error;
use share::hash::blake2b_256;

mod order;

// Alloc 4K fast HEAP + 2M HEAP to receives PrefilledData
default_alloc!(4 * 1024, 2048 * 1024, 64);

pub fn main() -> Result<(), Error> {
  let script = load_script()?;
  let args: Bytes = script.args().unpack();

  if args.len() != 32 {
    return Err(Error::InvalidArgument);
  }

  let input_position = QueryIter::new(load_cell, Source::Input)
        .position(|cell| &blake2b_256(cell.lock().as_slice())[..] == &args[..]);

  match input_position {
    None => return order::validate(),
    Some(position) => {
      match load_witness_args(position, Source::Input) {
        Ok(_) => Ok(()),
        Err(_) => Err(Error::WrongMatchInputWitness)
      }
    }
  }

}
