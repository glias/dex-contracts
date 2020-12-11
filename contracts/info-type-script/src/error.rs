use share::ckb_std::error::SysError;
use share::error::HelperError;

/// Error
#[repr(i8)]
#[derive(Debug)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing,
    LengthNotEnough,
    Encoding,
    MissingTypeScript = 5,
    OnlyOneLiquidityPool,
    AmountDiff,
    InfoCreationError,
    InvalidTypeID,
    VersionDiff = 10,
    InvalidLiquidityCell,
    UnknownLiquidity,
    InvalidInfoData,
    SellSUDTFailed,
    BuySUDTFailed = 15,
    InvalidChangeCell,
    LiquidityPoolTokenDiff,
    MintLiquidityFailed,
    BurnLiquidityFailed,
    VerifyPriceFailed = 20,
    InvalidFee,
    InvalidCKBAmount,
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

impl From<HelperError> for Error {
    fn from(err: HelperError) -> Self {
        match err {
            HelperError::MissingTypeScript => Self::MissingTypeScript,
        }
    }
}
