use share::ckb_std::{
    ckb_constants::Source,
    ckb_types::{packed::Transaction, prelude::*},
    // debug,
    high_level::{load_cell, load_script, load_transaction, QueryIter},
};
use share::error::Error;
use share::hash::new_blake2b;

pub fn verify_type_id(input: Transaction) -> Result<(), Error> {
    // TYPE_ID script should only accept one argument,
    // which is the hash of all inputs when creating
    // the cell.
    if load_script()?.args().len() != 32 {
        return Err(Error::InvalidTypeID);
    }

    // There could be at most one input cell and one
    // output cell with current TYPE_ID script.
    if QueryIter::new(load_cell, Source::GroupInput).count() > 1
        || QueryIter::new(load_cell, Source::GroupOutput).count() > 1
    {
        return Err(Error::InvalidTypeID);
    }

    // If there's only one output cell with current
    // TYPE_ID script, we are creating such a cell,
    // we also need to validate that the first argument matches
    // the hash of following items concatenated:
    // 1. Transaction hash of the first CellInput's OutPoint
    // 2. Cell index of the first CellInput's OutPoint
    // 3. Index of the first output cell in current script group.
    let tx = load_transaction()?;
    if QueryIter::new(load_cell, Source::GroupOutput).count() == 1 {
        let first_cell_input = QueryIter::new(load_cell, Source::GroupInput)
            .last()
            .ok_or(Error::InvalidTypeID)?;
        let first_output_index: u64 = 0;

        let mut blake2b = new_blake2b();
        blake2b.update(first_cell_input.as_slice());
        blake2b.update(&first_output_index.to_le_bytes());
        let mut ret = [0; 32];
        blake2b.finalize(&mut ret);

        if ret[..] != load_script()?.args().raw_data()[..] {
            return Err(Error::InvalidTypeID);
        }
    }
    Ok(())
}
