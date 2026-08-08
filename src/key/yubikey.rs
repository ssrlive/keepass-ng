use challenge_response::{
    ChallengeResponse,
    config::{Config, Mode, Slot},
};
use cipher::InvalidLength;
use hex::FromHexError;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::{crypt::CryptographyError, error::DatabaseKeyError, key::KeyElement};

fn parse_yubikey_slot(slot_number: &str) -> Result<Slot, ChallengeResponseKeyError> {
    if let Some(slot) = Slot::from_str(slot_number) {
        return Ok(slot);
    }
    Err(ChallengeResponseKeyError::InvalidSlot(slot_number.to_string()))
}

#[derive(Debug, Clone, PartialEq, Zeroize, ZeroizeOnDrop)]
pub enum ChallengeResponseKey {
    LocalChallenge(String),
    YubikeyChallenge(Yubikey, String),
}

#[derive(Debug, Clone, PartialEq, Zeroize, ZeroizeOnDrop)]
pub struct Yubikey {
    pub serial_number: u32,
    pub name: Option<String>,
}

impl ChallengeResponseKey {
    pub(crate) fn perform_challenge(&self, challenge: &[u8]) -> Result<KeyElement, ChallengeResponseKeyError> {
        match self {
            ChallengeResponseKey::LocalChallenge(secret) => {
                let secret_bytes = hex::decode(secret)?;

                let response = crate::crypt::calculate_hmac_sha1(&[challenge], &secret_bytes)?.to_vec();
                Ok(response)
            }
            ChallengeResponseKey::YubikeyChallenge(yubikey, slot_number) => {
                let mut challenge_response_client = ChallengeResponse::new()?;
                let slot = parse_yubikey_slot(slot_number)?;

                let yubikey_device = match challenge_response_client.find_device_from_serial(yubikey.serial_number) {
                    Ok(d) => d,
                    Err(_) => return Err(ChallengeResponseKeyError::KeyNotFound(yubikey.serial_number)),
                };

                let mut config = Config::new_from(yubikey_device);
                config = config.set_variable_size(true);
                config = config.set_mode(Mode::Sha1);
                config = config.set_slot(slot);

                match challenge_response_client.challenge_response_hmac(challenge, config) {
                    Ok(hmac_result) => Ok(hmac_result.to_vec()),
                    Err(e) => Err(ChallengeResponseKeyError::Api(e)),
                }
            }
        }
    }

    pub fn get_available_yubikeys() -> Result<Vec<Yubikey>, DatabaseKeyError> {
        let mut challenge_response_client = ChallengeResponse::new().map_err(ChallengeResponseKeyError::from)?;
        let mut response: Vec<Yubikey> = vec![];
        let yubikeys = match challenge_response_client.find_all_devices() {
            Ok(y) => y,
            Err(e) => return Err(DatabaseKeyError::from(ChallengeResponseKeyError::Api(e))),
        };
        for yubikey in yubikeys {
            let serial_number = match yubikey.serial {
                Some(n) => n,
                None => continue,
            };
            response.push(Yubikey {
                serial_number,
                name: yubikey.name,
            });
        }
        Ok(response)
    }

    pub fn get_yubikey(serial_number: Option<u32>) -> Result<Yubikey, DatabaseKeyError> {
        let all_yubikeys = ChallengeResponseKey::get_available_yubikeys()?;
        if all_yubikeys.is_empty() {
            return Err(DatabaseKeyError::from(ChallengeResponseKeyError::NoKeys));
        }

        let serial_number = match serial_number {
            Some(n) => n,
            None => {
                if all_yubikeys.len() != 1 {
                    return Err(DatabaseKeyError::from(ChallengeResponseKeyError::AmbiguousKeys));
                }
                return Ok(all_yubikeys[0].clone());
            }
        };

        for yubikey in all_yubikeys {
            if yubikey.serial_number == serial_number {
                return Ok(yubikey);
            }
        }
        Err(DatabaseKeyError::from(ChallengeResponseKeyError::KeyNotFound(serial_number)))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ChallengeResponseKeyError {
    #[error("No challenge-respone keys are connected to the system.")]
    NoKeys,

    #[error("Multiple challenge-response keys are connected to the system. Please provide a serial number.")]
    AmbiguousKeys,

    #[error("Challenge-response key with serial number {0} not found.")]
    KeyNotFound(u32),

    #[error("Invalid key slot: {0}")]
    InvalidSlot(String),

    #[error(transparent)]
    Api(#[from] challenge_response::error::ChallengeResponseError),

    #[error(transparent)]
    Cryptography(#[from] CryptographyError),

    #[error("Error decoding local challenge secret: {0}")]
    Hex(#[from] FromHexError),

    #[error("Local secret has invalid length")]
    InvalidLength(#[from] InvalidLength),

    #[error("Challenge-response authentication was not performed")]
    NotPerformed,
}
