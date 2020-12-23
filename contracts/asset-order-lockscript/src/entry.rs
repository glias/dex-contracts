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

use core::convert::TryFrom;
use core::result::Result;

use ckb_dyn_lock::locks::{
    CODE_HASH_SECP256K1_KECCAK256_SIGHASH_ALL, CODE_HASH_SECP256K1_KECCAK256_SIGHASH_ALL_DUAL,
};
use ckb_dyn_lock::DynLock;
use ckb_std::ckb_constants::{CellField, Source};
use ckb_std::ckb_types::packed::{Byte, Script, ScriptReader, WitnessArgs};
use ckb_std::ckb_types::{bytes::Bytes, prelude::*};
use ckb_std::dynamic_loading::CKBDLContext;
use ckb_std::error::SysError;
use ckb_std::high_level::{load_cell_lock_hash, load_script, load_witness_args, QueryIter};
use ckb_std::{default_alloc, syscalls};
use share::hash::blake2b_256;

use crate::error::Error;

// Alloc 4K fast HEAP + 2M HEAP to receives PrefilledData
default_alloc!(4 * 1024, 2048 * 1024, 64);

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let user_lock_hash: Bytes = script.args().unpack();

    // The length of user lock hash must be 32 bytes
    if user_lock_hash.len() != 32 {
        return Err(Error::WrongUserLockHashLength);
    }

    // Check cancellation
    // Firstly, we check whether there's a witness to cancel directly
    if let Ok(witness_args) = load_witness_args(0, Source::GroupInput) {
        return validate_witness(witness_args, user_lock_hash);
    }

    // Secondly, check whether there is an input's lock hash equal to this order lock args(user
    // lock hash). If it exists, verify it according to the process of cancellation.
    // if it does not exist, verify it according to the process of matching transaction.
    let input_position = QueryIter::new(load_cell_lock_hash, Source::Input)
        .position(|lock_hash| lock_hash == &user_lock_hash[..]);

    match input_position {
        None => return crate::order_validator::validate(),
        // Since anyone can pay lock dones't require signature to unlock, we must make
        // sure that witness args isn't empty.
        Some(position) if load_witness_args(position, Source::Input).is_ok() => Ok(()),
        _ => Err(Error::WrongMatchInputWitness),
    }
}

fn validate_witness(witness_args: WitnessArgs, user_lock_hash: Bytes) -> Result<(), Error> {
    // TODO: move user_lock_bytes into lock field
    let user_lock_bytes: Bytes = {
        let opt_bytes = witness_args.input_type();
        let user_lock = opt_bytes.to_opt().ok_or_else(|| Error::UserLockNotFound)?;
        user_lock.unpack()
    };
    ScriptReader::verify(&user_lock_bytes[..], false).map_err(|_| Error::UserLockScriptEncoding)?;

    let user_lock = Script::new_unchecked(user_lock_bytes);
    if &blake2b_256(user_lock.as_slice())[..] != &user_lock_hash[..] {
        return Err(Error::UserLockHashNotMatch);
    }

    let hash_type = HashType::try_from(user_lock.hash_type())?;
    let code_hash = user_lock.code_hash();
    let data_hash = match find_cell_dep(code_hash.unpack(), hash_type)? {
        Some(data_hash) => data_hash,
        // FIXME: Our forked pw-lock to verify signature, only personal hash is supported
        None if code_hash.unpack() == CODE_HASH_SECP256K1_KECCAK256_SIGHASH_ALL => {
            let alternative = CODE_HASH_SECP256K1_KECCAK256_SIGHASH_ALL_DUAL;
            match find_cell_dep(alternative, HashType::Data)? {
                Some(data_hash) => data_hash,
                None => return Err(Error::UserLockCellDepNotFound),
            }
        }
        _ => return Err(Error::UserLockCellDepNotFound),
    };

    let mut ctx = unsafe { CKBDLContext::<[u8; 128 * 1024]>::new() };
    let dyn_lock = DynLock::load(&mut ctx, &data_hash)?;

    let lock_args: Bytes = user_lock.args().unpack();
    dyn_lock.validate(&lock_args, lock_args.len() as u64)?;

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum HashType {
    Type,
    Data,
}

impl TryFrom<Byte> for HashType {
    type Error = Error;

    fn try_from(byte: Byte) -> Result<Self, Error> {
        let type_num: u8 = byte.into();
        match type_num {
            0 => Ok(HashType::Data),
            1 => Ok(HashType::Type),
            _ => Err(Error::UnknownUserLockHashType),
        }
    }
}

type DataHash = [u8; 32];

fn find_cell_dep(hash: [u8; 32], hash_type: HashType) -> Result<Option<DataHash>, Error> {
    let cell_field = match hash_type {
        HashType::Data => CellField::DataHash,
        HashType::Type => CellField::TypeHash,
    };

    let mut buf = [0u8; 32];
    for i in 0.. {
        match syscalls::load_cell_by_field(&mut buf, 0, i, Source::CellDep, cell_field) {
            Ok(_) => (),
            Err(SysError::IndexOutOfBound) => break,
            Err(err) => return Err(err.into()),
        };

        if hash != &buf[..] {
            continue;
        }

        return match hash_type {
            HashType::Type => {
                syscalls::load_cell_by_field(&mut buf, 0, i, Source::CellDep, CellField::DataHash)?;
                Ok(Some(buf))
            }
            HashType::Data => Ok(Some(hash)),
        };
    }

    Ok(None)
}
