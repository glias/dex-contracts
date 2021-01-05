use std::convert::From;

use ckb_tool::ckb_error::Error as CKBError;

#[derive(Clone, Debug)]
pub enum Kind {
    Transaction,
    Inputs,
    Contract,
}

#[derive(Clone, Debug)]
pub enum Error {
    ArgsErr(Kind),
    OpenFileErr(Kind),
    ReadFileErr(Kind),
    DecodeErr(Kind),
    BuildingScriptErr(String),
    MissingInputData,
    CkbErr(String),
}

impl From<CKBError> for Error {
    fn from(err: CKBError) -> Self {
        Error::CkbErr(err.to_string())
    }
}
