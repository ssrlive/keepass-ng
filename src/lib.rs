#![doc = include_str!("../README.md")]
#![recursion_limit = "1024"]

mod compression;
pub(crate) mod config;
pub(crate) mod crypt;
pub mod db;
pub(crate) mod error;
pub(crate) mod format;
pub(crate) mod hmac_block_stream;
#[cfg(feature = "save_kdbx4")]
mod io;
mod key;
pub(crate) mod variant_dictionary;
pub(crate) mod xml_db;

#[cfg(feature = "challenge_response")]
pub use self::key::ChallengeResponseKey;
pub use self::{
    config::DatabaseConfig,
    error::{BoxError, DatabaseIntegrityError, DatabaseKeyError, DatabaseOpenError, Error, Result},
    key::DatabaseKey,
};
pub use chrono::NaiveDateTime;
pub use uuid::Uuid;
