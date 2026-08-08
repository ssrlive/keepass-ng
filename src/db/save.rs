use crate::{DatabaseKey, db::types::Database, format::DatabaseVersion};

/// Errors occurring when saving a database.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseSaveError {
    #[error("Saving this database version is not supported")]
    UnsupportedVersion,

    #[error(transparent)]
    Serialization(#[from] quick_xml::SeError),

    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Key(#[from] crate::key::DatabaseKeyError),
    #[error(transparent)]
    Cryptography(#[from] crate::crypt::CryptographyError),
    #[error(transparent)]
    Random(#[from] getrandom::Error),
}

impl Database {
    /// Save a database to a `std::io::Write`
    #[cfg(feature = "save_kdbx4")]
    pub fn save(&self, destination: &mut dyn std::io::Write, key: DatabaseKey) -> Result<(), DatabaseSaveError> {
        use crate::format::kdbx4::dump_kdbx4;

        match self.config.version {
            DatabaseVersion::KDB(_) => Err(DatabaseSaveError::UnsupportedVersion),
            DatabaseVersion::KDB2(_) => Err(DatabaseSaveError::UnsupportedVersion),
            DatabaseVersion::KDB3(_) => Err(DatabaseSaveError::UnsupportedVersion),
            DatabaseVersion::KDB4(_) => dump_kdbx4(self, &key, destination),
        }
    }
}
