// Import from `core` instead of from `std` since we are in no-std mode
use core::result::Result;

// Import heap related library from `alloc`
// Import CKB syscalls and structures
// https://nervosnetwork.github.io/ckb-std/riscv64imac-unknown-none-elf/doc/ckb_std/index.html
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    // debug,
    dynamic_loading::CKBDLContext,
    high_level::{load_script, load_witness_args},
};

use blake2b_ref::{Blake2b, Blake2bBuilder};
use ckb_lib_secp256k1::LibSecp256k1;

use crate::error::Error;

fn new_blake2b() -> Blake2b {
    Blake2bBuilder::new(32)
        .personal(b"ckb-default-hash")
        .build()
}

fn test_validate_blake2b_sighash_all(
    lib: &LibSecp256k1,
    expected_pubkey_hash: &[u8],
) -> Result<(), Error> {
    let mut pubkey_hash = [0u8; 20];
    lib.validate_blake2b_sighash_all(&mut pubkey_hash)
        .map_err(|_| {
            // debug!("secp256k1 error {}", err_code);
            Error::Secp256k1
        })?;

    // compare with expected pubkey_hash
    if &pubkey_hash[..] != expected_pubkey_hash {
        return Err(Error::WrongPubkey);
    }
    Ok(())
}

pub fn validate() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();

    if args.len() != 20 {
        return Err(Error::Encoding);
    }

    let witness_args = load_witness_args(0, Source::GroupInput)?;

    // create a DL context with 128K buffer size
    let mut context = CKBDLContext::<[u8; 128 * 1024]>::new();
    let lib = LibSecp256k1::load(&mut context);

    if witness_args.input_type().to_opt().is_none() {
        test_validate_blake2b_sighash_all(&lib, &args)?;
    } else {
        let witness: Bytes = witness_args
            .input_type()
            .to_opt()
            .ok_or(Error::Encoding)?
            .unpack();
        let mut message = [0u8; 32];
        let mut signature = [0u8; 65];
        let msg_len = message.len();
        let sig_len = signature.len();
        assert_eq!(witness.len(), message.len() + signature.len());
        message.copy_from_slice(&witness[..msg_len]);
        signature.copy_from_slice(&witness[msg_len..msg_len + sig_len]);
        // recover pubkey_hash
        let prefilled_data = lib.load_prefilled_data().map_err(|_| {
            // debug!("load prefilled data error: {}", err);
            Error::LoadPrefilledData
        })?;
        let pubkey = lib
            .recover_pubkey(&prefilled_data, &signature, &message)
            .map_err(|_| {
                // debug!("recover pubkey error: {}", err);
                Error::RecoverPubkey
            })?;
        let pubkey_hash = {
            let mut buf = [0u8; 32];
            let mut hasher = new_blake2b();
            hasher.update(pubkey.as_slice());
            hasher.finalize(&mut buf);
            buf
        };
        if &args[..] != &pubkey_hash[..20] {
            return Err(Error::WrongPubkey);
        }
    }

    Ok(())
}
