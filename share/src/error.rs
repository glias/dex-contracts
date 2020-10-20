use ckb_std::error::SysError;

/// Error
#[repr(i8)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing,
    LengthNotEnough,
    Encoding,
    // Add customized errors here...
    Secp256k1 = 5,
    WrongPubkey,
    LoadPrefilledData,
    RecoverPubkey,
    WrongDataLengthOrFormat,
    WrongSUDTDiffAmount = 10,
    WrongDiffCapacity,
    WrongSUDTInputAmount,
    WrongOrderType,
    OrderPriceNotZero,
    InputsAndOutputsAmountNotSame = 15,
    WrongSwapAmount,
    TypeHashNotSame,
}

impl From<SysError> for Error {
    fn from(err: SysError) -> Self {
        use SysError::*;
        match err {
            IndexOutOfBound => Self::IndexOutOfBound,
            ItemMissing => Self::ItemMissing,
            LengthNotEnough(_) => Self::LengthNotEnough,
            Encoding => Self::Encoding,
            Unknown(err_code) => panic!("unexpected sys error {}", err_code),
        }
    }
}
