use crate::code_hashes::CODE_HASH_SECP256K1;
use crate::alloc::{alloc::{alloc, Layout}, boxed::Box};
use ckb_std::dynamic_loading::{CKBDLContext, Symbol};

/// function signature of validate_secp256k1_blake2b_sighash_all
type ValidateBlake2bSighashAll = unsafe extern "C" fn(pubkey_hash: *const u8) -> i32;
/// function signature of validate_signature
type ValidateSignature = unsafe extern "C" fn(
    prefilled_data: *const u8,
    signature_buffer: *const u8,
    signature_size: u64,
    message_buffer: *const u8,
    message_size: u64,
    output: *mut u8,
    output_len: *mut u64,
) -> i32;

/// function signature of load_prefilled_data
type LoadPrefilledData = unsafe extern "C" fn(data: *mut u8, len: *mut u64) -> i32;

/// Symbol name
const VALIDATE_BLAKE2B_SIGHASH_ALL: &[u8; 38] = b"validate_secp256k1_blake2b_sighash_all";
const VALIDATE_SIGNATURE: &[u8; 18] = b"validate_signature";
const LOAD_PREFILLED_DATA: &[u8; 19] = b"load_prefilled_data";

const SECP256K1_DATA_SIZE: usize = 1048576;
pub struct PrefilledData(Box<[u8; SECP256K1_DATA_SIZE]>);
pub struct Pubkey([u8; 33]);

impl Pubkey {
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
}

impl Default for Pubkey {
    fn default() -> Self {
        let inner = [0u8; 33];
        Pubkey(inner)
    }
}

impl Into<[u8; 33]> for Pubkey {
    fn into(self) -> [u8; 33] {
        self.0
    }
}
pub struct LibSecp256k1 {
    validate_blake2b_sighash_all: Symbol<ValidateBlake2bSighashAll>,
    validate_signature: Symbol<ValidateSignature>,
    load_prefilled_data: Symbol<LoadPrefilledData>,
}

impl LibSecp256k1 {
    pub fn load<T>(context: &mut CKBDLContext<T>) -> Self {
        // load library
        let lib = context.load(&CODE_HASH_SECP256K1).expect("load secp256k1");

        // find symbols
        let validate_blake2b_sighash_all: Symbol<ValidateBlake2bSighashAll> = unsafe {
            lib.get(VALIDATE_BLAKE2B_SIGHASH_ALL)
                .expect("load function")
        };
        let validate_signature: Symbol<ValidateSignature> =
            unsafe { lib.get(VALIDATE_SIGNATURE).expect("load function") };
        let load_prefilled_data: Symbol<LoadPrefilledData> =
            unsafe { lib.get(LOAD_PREFILLED_DATA).expect("load function") };
        LibSecp256k1 {
            validate_blake2b_sighash_all,
            load_prefilled_data,
            validate_signature,
        }
    }

    pub fn validate_blake2b_sighash_all(&self, pubkey_hash: &mut [u8; 20]) -> Result<(), i32> {
        let f = &self.validate_blake2b_sighash_all;
        let error_code = unsafe { f(pubkey_hash.as_mut_ptr()) };
        if error_code != 0 {
            return Err(error_code);
        }
        Ok(())
    }

    pub fn load_prefilled_data(&self) -> Result<PrefilledData, i32> {
        let mut data = unsafe {
            let layout = Layout::new::<[u8; SECP256K1_DATA_SIZE]>();
            let raw_allocation = alloc(layout) as *mut [u8; SECP256K1_DATA_SIZE];
            Box::from_raw(raw_allocation)
        };
        let mut len: u64 = SECP256K1_DATA_SIZE as u64;

        let f = &self.load_prefilled_data;
        let error_code = unsafe { f(data.as_mut_ptr(), &mut len as *mut u64) };
        if error_code != 0 {
            return Err(error_code);
        }
        Ok(PrefilledData(data))
    }

    pub fn recover_pubkey(
        &self,
        prefilled_data: &PrefilledData,
        signature: &[u8],
        message: &[u8],
    ) -> Result<Pubkey, i32> {
        let mut pubkey = Pubkey::default();
        let mut len: u64 = pubkey.0.len() as u64;

        let f = &self.validate_signature;
        let error_code = unsafe {
            f(
                prefilled_data.0.as_ptr(),
                signature.as_ptr(),
                signature.len() as u64,
                message.as_ptr(),
                message.len() as u64,
                pubkey.0.as_mut_ptr(),
                &mut len as *mut u64,
            )
        };
        if error_code != 0 {
            return Err(error_code);
        }
        debug_assert_eq!(pubkey.0.len() as u64, len);
        Ok(pubkey)
    }
}
