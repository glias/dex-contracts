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
    WrongUserLockHashLength,
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

    UserLockNotFound = 80,
    UserLockScriptEncoding,
    UserLockHashNotMatch,
    UnknownUserLockHashType,
    UserLockCellDepNotFound,
    ValidationFunctionNotFound,

    DynamicLoadingContextFailure = 90,
    DynamicLoadingInvalidElf,
    DynamicLoadingMemoryNotEnough,
    DynamicLoadingCellNotFound,
    DynamicLoadingInvalidAlign,
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

impl From<ckb_std::dynamic_loading::Error> for Error {
    fn from(err: ckb_std::dynamic_loading::Error) -> Self {
        use ckb_std::dynamic_loading::Error as DError;

        match err {
            DError::ContextFailure => Error::DynamicLoadingContextFailure,
            DError::InvalidElf => Error::DynamicLoadingInvalidElf,
            DError::MemoryNotEnough => Error::DynamicLoadingMemoryNotEnough,
            DError::CellNotFound => Error::DynamicLoadingCellNotFound,
            DError::InvalidAlign => Error::DynamicLoadingInvalidAlign,
            DError::Sys(err) => err.into(),
        }
    }
}

impl From<ckb_dyn_lock::Error> for Error {
    fn from(err: ckb_dyn_lock::Error) -> Self {
        use ckb_dyn_lock::Error as LError;

        match err {
            LError::DynamicLoading(e) => e.into(),
            LError::ValidationFunctionNotFound => Error::ValidationFunctionNotFound,
            LError::ValidateFailure(err_code) => {
                panic!("user lock validation failure {}", err_code)
            }
        }
    }
}
