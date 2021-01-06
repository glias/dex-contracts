#![no_std]

use ckb_std::dynamic_loading::{CKBDLContext, Symbol};

type Validate = unsafe extern "C" fn(args: *const u8, len: u64) -> i32;
const VALIDATE: &[u8; 8] = b"validate";

#[derive(Debug)]
pub enum Error {
    DynamicLoading(ckb_std::dynamic_loading::Error),
    ValidationFunctionNotFound,
    ValidateFailure(i32),
}

pub struct DynLock {
    validate: Symbol<Validate>,
}

impl DynLock {
    pub fn load<T>(context: &mut CKBDLContext<T>, code_hash: &[u8]) -> Result<Self, Error> {
        let lock = context.load(code_hash).map_err(Error::DynamicLoading)?;

        let validate: Symbol<Validate> = unsafe {
            lock.get(VALIDATE)
                .ok_or_else(|| Error::ValidationFunctionNotFound)?
        };

        Ok(DynLock { validate })
    }

    pub fn validate(&self, args: &[u8], args_size: u64) -> Result<(), Error> {
        let f = &self.validate;
        let error_code = unsafe { f(args.as_ptr(), args_size) };

        if error_code != 0 {
            return Err(Error::ValidateFailure(error_code));
        }
        Ok(())
    }
}
