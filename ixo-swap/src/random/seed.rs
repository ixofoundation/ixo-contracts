use base64ct::{Base64, Encoding};
use cosmwasm_std::Uint128;
use sha2::{Digest, Sha256};

pub fn get_seed(token_amounts: Uint128, block_height: u64) -> String {
    let mut sha256 = Sha256::new();
    sha256.update(token_amounts.to_le_bytes());
    sha256.update(block_height.to_le_bytes());
    let hash = sha256.finalize();
    Base64::encode_string(&hash)
}
