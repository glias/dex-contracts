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

mod entry;
mod error;
mod order_validator;

ckb_std::entry!(program_entry);

// Alloc 4K fast HEAP + 2M HEAP to receives PrefilledData
ckb_std::default_alloc!(4 * 1024, 2048 * 1024, 64);

/// program entry
fn program_entry() -> i8 {
    // Call main function and return error code
    match entry::main() {
        Ok(_) => 0,
        Err(err) => err as i8,
    }
}
