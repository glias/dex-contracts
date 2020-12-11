mod type_id;
mod verify;

use alloc::vec::Vec;
use core::result::Result;

// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
use share::cell::{InfoCellData, PoolCellData};
use share::ckb_std::{
    ckb_constants::Source,
    ckb_types::{
        packed::{Byte, CellOutput},
        prelude::*,
    },
    default_alloc,
    // debug,
    high_level::{load_cell, load_cell_data, load_cell_lock_hash, load_cell_type_hash, QueryIter},
};
use share::{blake2b, get_cell_type_hash};
use type_id::verify_type_id;

use crate::error::Error;

const INFO_TYPE_CODE_HASH: [u8; 32] = [2u8; 32];
const INFO_LOCK_CODE_HASH: [u8; 32] = [2u8; 32];
const POOL_BASE_CAPACITY: u128 = 16_200_000_000;

lazy_static::lazy_static! {
    static ref HASH_TYPE_DATA: Byte = Byte::new(1u8);
}

// Alloc 4K fast HEAP + 2M HEAP to receives PrefilledData
default_alloc!(4 * 1024, 2048 * 1024, 64);

pub fn main() -> Result<(), Error> {
    verify_type_id()?;

    let info_in_data = InfoCellData::from_raw(&load_cell_data(0, Source::Input)?)?;
    let pool_in_cell = load_cell(1, Source::Input)?;
    let pool_in_data = PoolCellData::from_raw(&load_cell_data(1, Source::Input)?)?;
    let info_out_cell = load_cell(0, Source::Output)?;
    let pool_out_cell = load_cell(1, Source::Output)?;
    let _pool_out_data = PoolCellData::from_raw(&load_cell_data(1, Source::Output)?)?;

    verify_info_creation(&info_out_cell, &pool_out_cell)?;

    if QueryIter::new(load_cell_type_hash, Source::Input)
        .map(|hash| hash == Some(INFO_TYPE_CODE_HASH))
        .count()
        != 1
        || QueryIter::new(load_cell_type_hash, Source::Output)
            .map(|hash| hash == Some(INFO_TYPE_CODE_HASH))
            .count()
            != 1
    {
        return Err(Error::OnlyOneLiquidityPool);
    }

    if (pool_in_cell.capacity().unpack() as u128) != POOL_BASE_CAPACITY + info_in_data.ckb_reserve
        || pool_in_data.sudt_amount != info_in_data.sudt_reserve
    {
        return Err(Error::AmountDiff);
    }

    if get_cell_type_hash(&load_cell(3, Source::Input)?)?.unpack()[0..20]
        == info_in_data.liquidity_sudt_type_hash
    {
        verify::verify_info_type_liquidity_tx()?;
    } else {
        verify::verify_info_type_asset_tx()?;
    }

    Ok(())
}

pub fn verify_info_creation(
    info_out_cell: &CellOutput,
    pool_out_cell: &CellOutput,
) -> Result<(), Error> {
    if QueryIter::new(load_cell_type_hash, Source::Input)
        .map(|hash| hash == Some(INFO_TYPE_CODE_HASH))
        .count()
        == 0
    {
        let info_out_lock_args: Vec<u8> = info_out_cell.lock().args().unpack();

        if QueryIter::new(load_cell_lock_hash, Source::Output)
            .map(|hash| hash == INFO_LOCK_CODE_HASH)
            .count()
            != 2
            || info_out_cell.lock().hash_type() != *HASH_TYPE_DATA
            || info_out_lock_args[0..20]
                != blake2b!("ckb", get_cell_type_hash(&pool_out_cell)?.unpack())[0..20]
            || info_out_lock_args[20..40] != get_cell_type_hash(&info_out_cell)?.unpack()[0..20]
            || info_out_cell.lock().code_hash().as_slice()
                != pool_out_cell.lock().code_hash().as_slice()
        {
            return Err(Error::InfoCreationError);
        }
    }

    Ok(())
}
