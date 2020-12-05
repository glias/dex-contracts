//! Generated by capsule
//!
//! `main.rs` is used to define rust lang items and modules.
//! See `entry.rs` for the `main` function.
//! See `error.rs` for the `Error` type.

#![no_std]
#![no_main]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]

use core::result::Result;

use share::ckb_std;
use share::ckb_std::{
    ckb_constants::Source,
    ckb_types::prelude::*,
    default_alloc,
    high_level::{load_cell, load_script, QueryIter},
};

use share::blake2b;
use share::error::Error;
use share::get_cell_type_hash;

default_alloc!(4 * 1024, 2048 * 1024, 64);

ckb_std::entry!(program_entry);

/// program entry
fn program_entry() -> i8 {
    // Call main function and return error code
    match main() {
        Ok(_) => 0,
        Err(err) => err as i8,
    }
}

fn main() -> Result<(), Error> {
    if QueryIter::new(load_cell, Source::GroupInput).count() != 2 {
        return Err(Error::InvalidInfoLock);
    }

    let info = load_cell(0, Source::GroupInput)?;
    let pool = load_cell(1, Source::GroupInput)?;
    let pool_type_hash = get_cell_type_hash(&pool)?;
    let self_args = load_script()?.args().as_slice();
    let hash = blake2b!("ckb", pool_type_hash.unpack());

    if hash[0..20] != self_args[0..20]
        || get_cell_type_hash(&info)?.as_slice()[0..20] != self_args[20..40]
    {
        return Err(Error::InvalidInfoLock);
    }

    Ok(())
}
