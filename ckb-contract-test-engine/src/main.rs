mod error;
mod types;
mod utils;

use clap::App;

use crate::error::{Error, Kind};
use crate::types::TestInfo;

const MAX_CYCLES: u64 = 100_000_000;

fn main() -> Result<(), Error> {
    let yml = clap::load_yaml!("test-engine.yml");
    let matches = App::from(yml).get_matches();
    let tx_path = matches
        .value_of("transaction")
        .ok_or(Error::ArgsErr(Kind::Transaction))?;
    let inputs_path = matches
        .value_of("input")
        .ok_or(Error::ArgsErr(Kind::Inputs))?;

    let test = TestInfo::new(tx_path, inputs_path)?;

    for contract in test.scripts.values() {
        let (mut ctx, tx) = if contract.is_type_script {
            utils::build_type_context(&test, contract.clone(), test.tx.clone())?
        } else {
            utils::build_lock_context(&test, contract.clone(), test.tx.clone())?
        };

        let tx = ctx.complete_tx(tx);
        ctx.verify_tx(&tx, MAX_CYCLES)?;
    }
    Ok(())
}
