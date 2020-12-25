mod error;
mod utils;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use ckb_tool::ckb_jsonrpc_types::{CellInfo, TransactionView};
use clap::App;

use crate::error::{Error, Kind};

const MAX_CYCLES: u64 = 100_000_000;

pub struct TestInfo {
    pub tx:          TransactionView,
    pub inputs_info: Vec<CellInfo>,
}

impl TestInfo {
    fn new(tx_path: &str, input_info_path: &str) -> Result<Self, Error> {
        let tx = serde_json::from_slice::<TransactionView>(
            &std::fs::read(tx_path).map_err(|_| Error::ReadFileErr(Kind::Transaction))?,
        )
        .map_err(|_| Error::DecodeErr(Kind::Transaction))?;
        let mut buf = String::new();
        let mut inputs_info = Vec::new();
        let mut raw_input_info = BufReader::new(
            File::open(input_info_path).map_err(|_| Error::OpenFileErr(Kind::Inputs))?,
        );
        let mut buf_size = usize::max_value();

        // Read input info file by line until the buffer size is 0.
        while buf_size != 0 {
            buf_size = raw_input_info
                .read_line(&mut buf)
                .map_err(|_| Error::ReadFileErr(Kind::Inputs))?;
            let info = serde_json::from_str::<CellInfo>(&buf)
                .map_err(|_| Error::DecodeErr(Kind::Inputs))?;
            inputs_info.push(info);
        }

        Ok(TestInfo { tx, inputs_info })
    }
}

fn main() -> Result<(), Error> {
    let yml = clap::load_yaml!("test-engine.yml");
    let matches = App::from(yml).get_matches();
    let tx_path = matches
        .value_of("transaction")
        .ok_or(Error::ArgsErr(Kind::Transaction))?;
    let inputs_path = matches
        .value_of("input")
        .ok_or(Error::ArgsErr(Kind::Inputs))?;
    let contracts_path = matches
        .values_of("contract")
        .ok_or(Error::ArgsErr(Kind::Contract))?
        .map(|path| Path::new(path).into())
        .collect::<Vec<PathBuf>>();

    let test = TestInfo::new(tx_path, inputs_path)?;

    for path in contracts_path.iter() {
        let (mut ctx, tx) = utils::build_vm_context(&test, path, test.tx.clone())?;
        let tx = ctx.complete_tx(tx);
        ctx.verify_tx(&tx, MAX_CYCLES)?;
    }
    Ok(())
}
