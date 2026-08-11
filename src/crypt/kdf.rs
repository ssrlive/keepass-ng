use aes::Aes256;
use cipher::{Array, typenum::U32};
use cipher::{BlockCipherEncrypt, KeyInit};
use sha2::{Digest, Sha256};

use super::CryptographyError;

pub(crate) trait Kdf {
    fn transform_key(&self, composite_key: &Array<u8, U32>) -> Result<Array<u8, U32>, CryptographyError>;
}

pub struct AesKdf {
    pub seed: Vec<u8>,
    pub rounds: u64,
}

impl Kdf for AesKdf {
    fn transform_key(&self, composite_key: &Array<u8, U32>) -> Result<Array<u8, U32>, CryptographyError> {
        let seed_arr = self.seed.as_slice().try_into().map_err(|_| cipher::InvalidLength)?;
        let cipher = Aes256::new(&seed_arr);
        let mut block1 = composite_key[0..16].try_into().map_err(|_| cipher::InvalidLength)?;
        let mut block2 = composite_key[16..].try_into().map_err(|_| cipher::InvalidLength)?;
        for _ in 0..self.rounds {
            cipher.encrypt_block(&mut block1);
            cipher.encrypt_block(&mut block2);
        }

        let mut digest = Sha256::new();

        digest.update(block1);
        digest.update(block2);

        Ok(digest.finalize())
    }
}

pub struct Argon2Kdf {
    pub memory: u64,
    pub salt: Vec<u8>,
    pub iterations: u64,
    pub parallelism: u32,
    pub version: argon2::Version,
    pub variant: argon2::Variant,
}

impl Kdf for Argon2Kdf {
    fn transform_key(&self, composite_key: &Array<u8, U32>) -> Result<Array<u8, U32>, CryptographyError> {
        // Disable Argon2 multithreading on wasm32 targets to avoid panics on platforms without thread support.
        let thread_mode = if cfg!(target_arch = "wasm32") {
            argon2::ThreadMode::Sequential
        } else {
            argon2::ThreadMode::Parallel
        };

        #[allow(clippy::cast_possible_truncation)]
        let config = argon2::Config {
            ad: &[],
            hash_length: 32,
            lanes: self.parallelism,
            mem_cost: (self.memory / 1024) as u32,
            secret: &[],
            thread_mode,
            time_cost: self.iterations as u32,
            variant: self.variant,
            version: self.version,
        };

        let key = argon2::hash_raw(composite_key, &self.salt, &config)?;

        key.as_slice().try_into().map_err(|_| cipher::InvalidLength.into())
    }
}

#[cfg(test)]
mod tests {
    use argon2::{Config, ThreadMode, Variant, Version, hash_raw};
    use cipher::{Array, consts::U32};

    use super::{Argon2Kdf, Kdf};

    #[test]
    fn argon2_memory_is_interpreted_as_bytes() {
        let composite_key = Array::<u8, U32>::from([7_u8; 32]);
        let salt = vec![9_u8; 32];
        let kdf = Argon2Kdf {
            memory: 64 * 1024,
            salt: salt.clone(),
            iterations: 1,
            parallelism: 1,
            version: Version::Version13,
            variant: Variant::Argon2id,
        };

        let expected = hash_raw(
            &composite_key,
            &salt,
            &Config {
                thread_mode: ThreadMode::Parallel,
                ad: &[],
                hash_length: 32,
                lanes: 1,
                mem_cost: 64,
                secret: &[],
                time_cost: 1,
                variant: Variant::Argon2id,
                version: Version::Version13,
            },
        )
        .expect("argon2 test vector should derive successfully");

        let actual = kdf.transform_key(&composite_key).expect("argon2 kdf should derive successfully");

        assert_eq!(actual.as_slice(), expected.as_slice());
    }
}
