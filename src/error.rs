//! Error types that this crate can return

pub use crate::config::{CompressionConfigError, InnerCipherConfigError, KdfConfigError, OuterCipherConfigError};
pub use crate::crypt::CryptographyError;
#[cfg(feature = "save_kdbx4")]
pub use crate::db::DatabaseSaveError;
pub use crate::db::ParseColorError;
pub use crate::db::{DatabaseIntegrityError, DatabaseOpenError};
pub use crate::format::DatabaseVersionParseError;
pub use crate::format::hmac_block_stream::BlockStreamError;
pub use crate::format::kdb::KdbOpenError;
pub use crate::format::kdbx3::{Kdbx3OpenError, Kdbx3OuterHeaderError};
pub use crate::format::kdbx4::{Kdbx4InnerHeaderError, Kdbx4OpenError, Kdbx4OuterHeaderError};
pub use crate::format::variant_dictionary::VariantDictionaryError;
pub use crate::format::xml_db::parse::XmlParseError;
#[cfg(feature = "challenge_response")]
pub use crate::key::ChallengeResponseKeyError;
pub use crate::key::DatabaseKeyError;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("std::io::Error {0}")]
    Io(#[from] std::io::Error),

    #[error("DatabaseError::RecycleBinDisabled")]
    RecycleBinDisabled,

    #[error("DatabaseError::RecycleBinAlreadyExists")]
    RecycleBinAlreadyExists,

    #[error("DatabaseOpenError {0}")]
    DatabaseOpenError(#[from] DatabaseOpenError),

    #[cfg(feature = "save_kdbx4")]
    #[error("DatabaseSaveError {0}")]
    DatabaseSaveError(#[from] DatabaseSaveError),

    #[cfg(feature = "totp")]
    #[error("DbOtpError {0}")]
    DbOtpError(#[from] crate::db::otp::TOTPError),

    #[error("OuterCipherConfigError {0}")]
    OuterCipherConfigError(#[from] OuterCipherConfigError),

    #[error("InnerCipherConfigError {0}")]
    InnerCipherConfigError(#[from] InnerCipherConfigError),

    #[error("CompressionConfigError {0}")]
    CompressionConfigError(#[from] CompressionConfigError),

    #[error("KdfConfigError {0}")]
    KdfConfigError(#[from] KdfConfigError),

    #[error("CryptographyError {0}")]
    CryptographyError(#[from] CryptographyError),

    #[error("BlockStreamError {0}")]
    BlockStreamError(#[from] BlockStreamError),

    #[error("VariantDictionaryError {0}")]
    VariantDictionaryError(#[from] VariantDictionaryError),

    #[error("XmlParseError {0}")]
    XmlParseError(#[from] XmlParseError),

    #[error("ParseColorError {0}")]
    ParseColorError(#[from] ParseColorError),

    #[error("ParseIconIdError {}", icon_id)]
    ParseIconIdError { icon_id: usize },

    #[cfg(feature = "merge")]
    #[error("MergeError {0}")]
    MergeError(#[from] crate::db::merge::MergeError),

    #[error("String error: {0}")]
    String(String),
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::String(s.to_string())
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::String(s)
    }
}

impl From<&String> for Error {
    fn from(s: &String) -> Self {
        Error::String(s.clone())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

// move error type conversions to a module and exclude them from coverage counting.
mod conversions {
    use super::{
        BlockStreamError, CompressionConfigError, CryptographyError, DatabaseIntegrityError, DatabaseOpenError, InnerCipherConfigError,
        KdfConfigError, OuterCipherConfigError, VariantDictionaryError, XmlParseError,
    };

    impl From<CryptographyError> for DatabaseOpenError {
        fn from(e: CryptographyError) -> Self {
            DatabaseIntegrityError::from(e).into()
        }
    }

    impl From<BlockStreamError> for DatabaseOpenError {
        fn from(e: BlockStreamError) -> Self {
            DatabaseIntegrityError::from(e).into()
        }
    }

    impl From<XmlParseError> for DatabaseOpenError {
        fn from(e: XmlParseError) -> Self {
            DatabaseIntegrityError::from(e).into()
        }
    }

    impl From<InnerCipherConfigError> for DatabaseOpenError {
        fn from(e: InnerCipherConfigError) -> Self {
            DatabaseIntegrityError::from(e).into()
        }
    }

    impl From<OuterCipherConfigError> for DatabaseOpenError {
        fn from(e: OuterCipherConfigError) -> Self {
            DatabaseIntegrityError::from(e).into()
        }
    }

    impl From<KdfConfigError> for DatabaseOpenError {
        fn from(e: KdfConfigError) -> Self {
            DatabaseIntegrityError::from(e).into()
        }
    }

    impl From<VariantDictionaryError> for DatabaseOpenError {
        fn from(e: VariantDictionaryError) -> Self {
            DatabaseIntegrityError::from(e).into()
        }
    }

    impl From<CompressionConfigError> for DatabaseOpenError {
        fn from(e: CompressionConfigError) -> Self {
            DatabaseIntegrityError::from(e).into()
        }
    }
}
