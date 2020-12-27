extern crate alloc;

#[path = "../../contracts/asset-order-lockscript/src/order_validator.rs"]
mod order_validator;

fn main() {
    if let Err(err) = order_validator::validate() {
        std::process::exit(err as i32);
    }
}
