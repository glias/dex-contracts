use ckb_std::error::SysError;

// TODO: remove unused error?
/// Error
#[repr(i8)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing,
    LengthNotEnough,
    Encoding,

    // Input Order
    WrongUserLockHashSize = 5,
    WrongOrderDataSize,
    OrderPriceIsZero,
    UnknownOrderType,
    UnexpectedOrderVersion = 9,

    // Order deal
    UnknownOutputLock = 10,
    OutputTypeHashChanged,
    OutputOrderPriceChanged,
    OutputOrderTypeChanged,
    OutputOrderDataSizeChanged,
    OrderAmountIsZero = 15,
    OutputNotASudtCell, // Data size should be equal or more than 16
    OutputNotAFreeCell, // Data size should be zero
    BuyCKBOrderSudtAmountIsZero,
    OutputSudtAmountIsZero,
    OutputBurnSudtAmount = 20,
    NegativeSudtDifference,
    NegativeCapacityDifference,
    PriceMismatch,
    OrderStillMatchable = 24,

    // Directly cancellation
    CancelOrderWithoutWitness = 25,
    UserLockNotFound,
    UserLockScriptEncoding,
    UserLockHashNotMatch,
    UnknownUserLockHashType,
    UserLockCellDepNotFound = 30,
    ValidationFunctionNotFound,
    DynamicLoadingContextFailure,
    DynamicLoadingInvalidElf,
    DynamicLoadingMemoryNotEnough,
    DynamicLoadingCellNotFound = 35,
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
