use std::fmt;
use std::sync::Arc;

#[cfg(feature = "challenge_response_usb")]
use challenge_response::{
    ChallengeResponse,
    config::{Config, Mode, Slot},
};
use cipher::InvalidLength;
use hex::FromHexError;
use zeroize::Zeroize;
#[cfg(feature = "challenge_response_usb")]
use zeroize::ZeroizeOnDrop;

#[cfg(feature = "challenge_response_usb")]
use crate::error::DatabaseKeyError;
use crate::{crypt::CryptographyError, key::KeyElement};

#[cfg(feature = "challenge_response_usb")]
fn parse_yubikey_slot(slot_number: &str) -> Result<Slot, ChallengeResponseKeyError> {
    if let Some(slot) = Slot::from_str(slot_number) {
        return Ok(slot);
    }
    Err(ChallengeResponseKeyError::InvalidSlot(slot_number.to_string()))
}

/// A function that answers challenges on behalf of a key this crate cannot
/// reach itself — a key whose secret lives on another device.
///
/// It is called synchronously with the challenge exactly where
/// [`ChallengeResponseKey::LocalChallenge`] or a YubiKey would be
/// challenged: when opening a database (with the KDF seed read from the
/// file's outer header) and once per save (with the freshly generated
/// seed). It may block while the answer is obtained — a hardware key
/// blocks on a touch in the same place. The secret itself never passes
/// through this crate.
pub type ChallengeResponder = Arc<dyn Fn(&[u8]) -> Result<KeyElement, ChallengeResponseKeyError> + Send + Sync>;

#[derive(Clone)]
pub enum ChallengeResponseKey {
    LocalChallenge(String),
    #[cfg(feature = "challenge_response_usb")]
    YubikeyChallenge(Yubikey, String),
    /// The answer is supplied by a [`ChallengeResponder`].
    External(ChallengeResponder),
}

// A responder is neither debug-printable nor zeroizable as a value (it is
// shared, and owns its own hygiene), so the trait implementations are
// written out rather than derived. The existing variants keep the exact
// behaviour the derives gave them.
impl fmt::Debug for ChallengeResponseKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChallengeResponseKey::LocalChallenge(secret) => f.debug_tuple("LocalChallenge").field(secret).finish(),
            #[cfg(feature = "challenge_response_usb")]
            ChallengeResponseKey::YubikeyChallenge(yubikey, slot) => f.debug_tuple("YubikeyChallenge").field(yubikey).field(slot).finish(),
            ChallengeResponseKey::External(_) => f.write_str("External(<responder>)"),
        }
    }
}

impl PartialEq for ChallengeResponseKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ChallengeResponseKey::LocalChallenge(a), ChallengeResponseKey::LocalChallenge(b)) => a == b,
            #[cfg(feature = "challenge_response_usb")]
            (ChallengeResponseKey::YubikeyChallenge(a, a_slot), ChallengeResponseKey::YubikeyChallenge(b, b_slot)) => {
                a == b && a_slot == b_slot
            }
            (ChallengeResponseKey::External(a), ChallengeResponseKey::External(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Zeroize for ChallengeResponseKey {
    fn zeroize(&mut self) {
        match self {
            ChallengeResponseKey::LocalChallenge(secret) => secret.zeroize(),
            #[cfg(feature = "challenge_response_usb")]
            ChallengeResponseKey::YubikeyChallenge(yubikey, slot) => {
                yubikey.zeroize();
                slot.zeroize();
            }
            // Whatever a responder holds is zeroized by its own drop; it
            // may also be shared with other keys.
            ChallengeResponseKey::External(_) => {}
        }
    }
}

impl Drop for ChallengeResponseKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[cfg(feature = "challenge_response_usb")]
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
            ChallengeResponseKey::External(respond) => respond(challenge),
            #[cfg(feature = "challenge_response_usb")]
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

    #[cfg(feature = "challenge_response_usb")]
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

    #[cfg(feature = "challenge_response_usb")]
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

    #[cfg(feature = "challenge_response_usb")]
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

    /// A [`ChallengeResponseKey::External`] responder could not or would
    /// not answer. The message comes from the responder.
    #[error("External challenge-response failed: {0}")]
    External(String),
}

#[cfg(all(test, feature = "save_kdbx4"))]
mod external_tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::db::{Database, Entry, Group, NodeIterator, node::*};
    use crate::key::DatabaseKey;

    /// A standing "other device": every challenge is answered with a
    /// response derived from a secret only it knows, and the challenges it
    /// saw are recorded.
    #[derive(Clone, Default)]
    struct RemoteKey {
        seen: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl RemoteKey {
        fn key(self) -> ChallengeResponseKey {
            ChallengeResponseKey::External(Arc::new(move |challenge| {
                self.seen.lock().unwrap().push(challenge.to_vec());
                Ok(crate::crypt::calculate_hmac_sha1(&[challenge], b"remote-only secret")?.to_vec())
            }))
        }
    }

    fn sample_database() -> Database {
        let mut db = Database::new(crate::DatabaseConfig::default());
        let mut root = Group::new("Root");
        root.add_child(rc_refcell_node(Entry::default()), 0);
        db.root = rc_refcell_node(root).into();
        db
    }

    #[test]
    fn external_key_is_challenged_with_the_file_seeds() {
        let remote = RemoteKey::default();
        let key = DatabaseKey::new()
            .with_password("pw")
            .with_challenge_response_key(remote.clone().key());

        let mut file = Vec::new();
        sample_database().save(&mut file, key.clone()).unwrap();

        let db = Database::parse(&file, key).unwrap();
        assert_eq!(NodeIterator::new(&db.root).filter(node_is_entry).count(), 1);

        // Exactly two challenges: the seed written by the save, then the
        // same seed read back from the file's outer header by the open.
        let seen = remote.seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].len(), 32, "the challenge is the 32-byte KDF seed");
        assert_eq!(seen[0], seen[1]);
    }

    #[test]
    fn a_declining_responder_fails_the_save_and_the_open() {
        let declining = || ChallengeResponseKey::External(Arc::new(|_| Err(ChallengeResponseKeyError::External("declined".into()))));

        let mut file = Vec::new();
        let err = sample_database()
            .save(
                &mut file,
                DatabaseKey::new().with_password("pw").with_challenge_response_key(declining()),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("declined"),
            "the responder's reason reaches the caller: {err}"
        );
        assert!(file.is_empty(), "nothing is written before the challenge");

        // A file that does need a challenge response cannot be opened by a
        // responder that declines.
        let mut enrolled = Vec::new();
        sample_database()
            .save(
                &mut enrolled,
                DatabaseKey::new()
                    .with_password("pw")
                    .with_challenge_response_key(ChallengeResponseKey::LocalChallenge(
                        "0102030405060708090a0b0c0d0e0f1011121314".to_string(),
                    )),
            )
            .unwrap();
        let err = Database::parse(
            &enrolled,
            DatabaseKey::new().with_password("pw").with_challenge_response_key(declining()),
        )
        .unwrap_err();
        assert!(err.to_string().contains("declined"), "{err}");
    }

    #[test]
    fn a_wrong_answer_is_an_incorrect_key() {
        // A responder holding the wrong secret opens nothing: the answer is
        // part of the composite key like any other challenge response.
        let enrolled = DatabaseKey::new()
            .with_password("pw")
            .with_challenge_response_key(ChallengeResponseKey::External(Arc::new(|challenge| {
                Ok(crate::crypt::calculate_hmac_sha1(&[challenge], b"right secret")?.to_vec())
            })));
        let mut file = Vec::new();
        sample_database().save(&mut file, enrolled).unwrap();

        let wrong = DatabaseKey::new()
            .with_password("pw")
            .with_challenge_response_key(ChallengeResponseKey::External(Arc::new(|challenge| {
                Ok(crate::crypt::calculate_hmac_sha1(&[challenge], b"wrong secret")?.to_vec())
            })));
        assert!(Database::parse(&file, wrong).is_err());
    }

    #[test]
    fn external_variant_traits() {
        let responder: ChallengeResponder = Arc::new(|_| Ok(vec![0u8; 20]));
        let a = ChallengeResponseKey::External(Arc::clone(&responder));
        let b = ChallengeResponseKey::External(Arc::clone(&responder));
        let c = ChallengeResponseKey::External(Arc::new(|_| Ok(vec![0u8; 20])));
        // Clones share the responder; distinct responders are distinct keys.
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, ChallengeResponseKey::LocalChallenge("00".into()));
        assert_eq!(format!("{a:?}"), "External(<responder>)");
        // The existing variants keep the derived behaviour.
        assert_eq!(
            format!("{:?}", ChallengeResponseKey::LocalChallenge("0102".into())),
            r#"LocalChallenge("0102")"#
        );
    }
}
