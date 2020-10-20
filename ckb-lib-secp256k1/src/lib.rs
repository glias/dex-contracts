#![no_std]

extern crate alloc;

mod code_hashes;
mod libsecp256k1;

pub use code_hashes::CODE_HASH_SECP256K1;
pub use libsecp256k1::LibSecp256k1;