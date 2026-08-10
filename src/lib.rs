#![doc = include_str!("../README.md")]
#![recursion_limit = "1024"]

mod compression;
pub(crate) mod config;
pub(crate) mod crypt;
pub mod db;
pub(crate) mod error;
pub(crate) mod format;
mod key;

#[cfg(feature = "challenge_response")]
pub use self::error::ChallengeResponseKeyError;
#[cfg(feature = "challenge_response")]
pub use self::key::ChallengeResponseKey;
pub use self::{
    config::DatabaseConfig,
    error::{
        BoxError, DatabaseIntegrityError, DatabaseKeyError, DatabaseOpenError, DatabaseVersionParseError, Error, KdbOpenError,
        Kdbx3OpenError, Kdbx3OuterHeaderError, Kdbx4InnerHeaderError, Kdbx4OpenError, Kdbx4OuterHeaderError, Result,
    },
    key::DatabaseKey,
};
pub use chrono::NaiveDateTime;
pub use config::DatabaseVersion;
pub use uuid::Uuid;
