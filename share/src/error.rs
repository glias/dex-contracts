use ckb_std::error::SysError;

/// Error
#[repr(i8)]
#[derive(Debug)]
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
    WrongSwapAmount = 15,
    TypeHashNotSame,
    OrderPriceNotSame,
    LockHashNotSame,
    InvalidArgument,
    NoInputLockHashMatch = 20,
    WrongMatchInputWitness,
    InvalidLiquidityDataLen,
    InvalidLiquidityData,
    InvalidEncodeNumber,
    LiquiditySUDTTypeHashMismatch = 25,
    PackingMixin,
    InvalidOrderKind,
    InvalidTypeID,
    InvalidCodeHash,
    InvalidTypeHash = 30,
    OnlyOneLiquidityPool,
    InvalidCount,
    ImpossibleAction,
    LiquidityAction,
    InvalidFee = 35,
    InvalidData,
    InputCellError,
    OutputCellError,
    PoolNotFound,
    MissingTypeScript = 40,
    SellCkbFailed,
    BuyCkbFailed,
    AddLiquidityFailed,
    RemoveLiquidityFailed,
    AddOverflow,
    SubOverflow,
    MultiplOverflow,
    DivideOverflow,
    ///
    NoInfoCell,
    InvalidInfoLock,
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
