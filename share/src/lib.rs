#![no_std]
#![feature(lang_items)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]

pub use blake2b_ref;
pub use ckb_lib_secp256k1;
pub use ckb_std;

pub mod cell;
pub mod constants;
pub mod error;
pub mod hash;
pub mod signature;

use ckb_std::{
    ckb_constants::Source,
    ckb_types::packed::{Byte32, CellOutput},
    high_level::{load_cell_lock_hash, load_cell_type_hash},
};
use error::Error;

#[macro_export]
macro_rules! blake2b {
    ($($field: expr), *) => {{
        let mut res = [0u8; 32];
        let blake2b = share::hash::new_blake2b();

        $( blake2b.update($field.as_ref()); )*

        blake2b.finalize(&mut res);
        res
    }}
}

pub fn get_cell_type_hash(cell: &CellOutput) -> Result<Byte32, Error> {
    let script = cell.type_().to_opt().ok_or(Error::MissingTypeScript)?;
    Ok(script.code_hash())
}

pub fn check_args_len(expected: usize, actual: usize) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::Encoding);
    }
    Ok(())
}

pub fn decode_u128(data: &[u8]) -> Result<u128, Error> {
    if data.len() != 16 {
        return Err(Error::InvalidEncodeNumber);
    }

    let mut buf = [0u8; 16];

    buf.copy_from_slice(data);
    Ok(u128::from_le_bytes(buf))
}

pub fn decode_u64(data: &[u8]) -> Result<u64, Error> {
    if data.len() != 8 {
        return Err(Error::InvalidEncodeNumber);
    }

    let mut buf = [0u8; 8];
    buf.copy_from_slice(data);
    Ok(u64::from_le_bytes(buf))
}

pub fn decode_u8(data: &[u8]) -> Result<u8, Error> {
    if data.len() != 1 {
        return Err(Error::InvalidEncodeNumber);
    }

    let mut buf = [0u8; 1];
    buf.copy_from_slice(data);
    Ok(u8::from_le_bytes(buf))
}

pub fn decode_i8(data: &[u8]) -> Result<i8, Error> {
    if data.len() != 1 {
        return Err(Error::InvalidEncodeNumber);
    }

    let mut buf = [0u8; 1];
    buf.copy_from_slice(data);
    Ok(i8::from_le_bytes(buf))
}

pub fn check_lock_hash(index: usize) -> Result<(), Error> {
    let input_lock_hash = load_cell_lock_hash(index, Source::Input)?;
    let output_lock_hash = load_cell_lock_hash(index, Source::Output)?;
    if input_lock_hash != output_lock_hash {
        return Err(Error::LockHashNotSame);
    }
    Ok(())
}

pub fn check_type_hash(index: usize) -> Result<(), Error> {
    let input_type_hash = load_cell_type_hash(index, Source::Input)?;
    let output_type_hash = load_cell_type_hash(index, Source::Output)?;
    if input_type_hash != output_type_hash {
        return Err(Error::TypeHashNotSame);
    }
    Ok(())
}
