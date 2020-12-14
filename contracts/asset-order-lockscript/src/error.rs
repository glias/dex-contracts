use ckb_std::error::SysError;

// TODO: remove unused error?
/// Error
#[repr(i8)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing,
    LengthNotEnough,
    Encoding,
    // Add customized errors here...
    #[allow(dead_code)]
    Secp256k1 = 5,
    #[allow(dead_code)]
    WrongPubkey,
    #[allow(dead_code)]
    LoadPrefilledData,
    #[allow(dead_code)]
    RecoverPubkey,
    WrongDataLengthOrFormat,
    NegativeSudtDifference = 10,
    NegativeCapacityDifference,
    InputSudtIsZero,
    UnknownOrderType,
    PriceIsZero,
    PriceNotMatched = 15,
    TypeHashChanged,
    PriceChanged,
    UnknownLock,
    InvalidArgument,
    #[allow(dead_code)]
    NoInputLockHashMatch = 20,
    WrongMatchInputWitness,
    PriceExponentOutOfRange, // -100 ~ 100
    OrderAmountIsZero,
    UnmatchableOrder,
    NotASudtCell = 25,
    NotAFreeCell,
    UnexpectedVersion,
    OrderTypeChanged,
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
