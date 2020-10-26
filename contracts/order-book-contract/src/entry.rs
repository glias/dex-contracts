use core::result::Result;

use ckb_std::{
  default_alloc,
  ckb_constants::Source,
  high_level::load_witness_args,
};

use share::error::Error;

mod order;

// Alloc 4K fast HEAP + 2M HEAP to receives PrefilledData
default_alloc!(4 * 1024, 2048 * 1024, 64);

pub fn main() -> Result<(), Error> {
  return match load_witness_args(0, Source::GroupInput) {
    Ok(_) => Ok(()),
    Err(_) => order::validate(),
  };

}
