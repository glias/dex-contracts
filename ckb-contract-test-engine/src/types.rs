use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use ckb_tool::ckb_jsonrpc_types::{CellInfo, TransactionView};
use ckb_tool::ckb_types::bytes::Bytes;
use ckb_tool::ckb_types::packed::Byte32;
use ckb_tool::ckb_types::prelude::*;
use serde::{Deserialize, Serialize};

use crate::blake2b;
use crate::error::{Error, Kind};

pub struct TestInfo {
    pub tx:          TransactionView,
    pub inputs_info: Vec<CellInfo>,
    pub scripts:     HashMap<Byte32, Contract>,
}

impl TestInfo {
    #[allow(clippy::mutable_key_type)]
    pub fn new(tx_path: &str, input_info_path: &str) -> Result<Self, Error> {
        let mut raw_tx =
            BufReader::new(File::open(tx_path).map_err(|_| Error::OpenFileErr(Kind::Transaction))?);
        let mut buf = String::new();
        let _tx_size = raw_tx
            .read_line(&mut buf)
            .map_err(|_| Error::ReadFileErr(Kind::Transaction))?;
        let tx = serde_json::from_str::<TransactionView>(&buf)
            .map_err(|_| Error::DecodeErr(Kind::Transaction))?;
        let _script_size = raw_tx
            .read_line(&mut buf)
            .map_err(|_| Error::ReadFileErr(Kind::Contract))?;
        let scripts = serde_json::from_str::<Scripts>(&buf)
            .map_err(|_| Error::ReadFileErr(Kind::Transaction))?
            .to_contract_map();

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

        Ok(TestInfo {
            tx,
            inputs_info,
            scripts,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Contract {
    pub name:           String,
    pub bin:            Bytes,
    pub is_type_script: bool,
}

#[derive(Deserialize, Serialize)]
pub struct Scripts {
    inner: Vec<ScriptInfo>,
}

impl Scripts {
    #[allow(clippy::mutable_key_type)]
    pub fn to_contract_map(&self) -> HashMap<Byte32, Contract> {
        self.inner
            .iter()
            .map(|s| {
                let path = Path::new(&s.path);
                let bin = Bytes::from(std::fs::read(path).expect(""));

                let contract = Contract {
                    name:           path
                        .to_path_buf()
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned(),
                    is_type_script: s.is_type_script,
                    bin:            bin.clone(),
                };
                (blake2b!(bin).pack(), contract)
            })
            .collect::<HashMap<Byte32, _>>()
    }
}

#[derive(Deserialize, Serialize)]
pub struct ScriptInfo {
    path:           String,
    is_type_script: bool,
}
