extern crate alloc;

#[path = "../../contracts/asset-order-lockscript/src/entry.rs"]
mod entry;
#[path = "../../contracts/asset-order-lockscript/src/error.rs"]
mod error;
#[path = "../../contracts/asset-order-lockscript/src/order_validator.rs"]
mod order_validator;

fn main() {
    if let Err(err) = entry::main() {
        std::process::exit(err as i32);
    }
}
