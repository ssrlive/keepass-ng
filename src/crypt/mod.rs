#[cfg(feature = "challenge_response")]
use cipher::typenum::U20;

use cipher::{
    Array,
    typenum::{U32, U64},
};

use hmac::{Hmac, KeyInit, Mac};
#[cfg(feature = "challenge_response")]
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

/// Errors while performing cryptographic operations.
#[derive(Debug, thiserror::Error)]
pub enum CryptographyError {
    #[error(transparent)]
    InvalidLength(#[from] cipher::InvalidLength),

    #[error(transparent)]
    Unpadding(#[from] cipher::block_padding::Error),

    #[error(transparent)]
    Padding(#[from] cipher::inout::PadError),

    #[error(transparent)]
    Argon2(#[from] argon2::Error),
}

pub(crate) mod ciphers;
pub(crate) mod kdf;

pub(crate) fn calculate_hmac(elements: &[&[u8]], key: &[u8]) -> Result<Array<u8, U32>, CryptographyError> {
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key)?;

    for element in elements {
        mac.update(element);
    }

    let result = mac.finalize();
    Ok(result.into_bytes())
}

#[cfg(feature = "challenge_response")]
pub(crate) fn calculate_hmac_sha1(elements: &[&[u8]], key: &[u8]) -> Result<Array<u8, U20>, CryptographyError> {
    type HmacSha1 = Hmac<Sha1>;
    let mut mac = HmacSha1::new_from_slice(key)?;

    for element in elements {
        mac.update(element);
    }

    let result = mac.finalize();
    Ok(result.into_bytes())
}

pub(crate) fn calculate_sha256(elements: &[&[u8]]) -> Array<u8, U32> {
    let mut digest = Sha256::new();

    for element in elements {
        digest.update(element);
    }

    digest.finalize()
}

pub(crate) fn calculate_sha512(elements: &[&[u8]]) -> Array<u8, U64> {
    let mut digest = Sha512::new();

    for element in elements {
        digest.update(element);
    }

    digest.finalize()
}
