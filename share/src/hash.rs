use blake2b_ref::{Blake2b, Blake2bBuilder};

pub fn new_blake2b() -> Blake2b {
    Blake2bBuilder::new(32)
        .personal(b"ckb-default-hash")
        .build()
}

pub fn calc_blake2b_hash(message: &[u8]) -> [u8; 32] {
  let mut blake2b = new_blake2b();
  blake2b.update(message);
  let mut hash = [0u8; 32];
  blake2b.finalize(&mut hash[..]);
  hash
}



