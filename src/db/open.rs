use crate::db::types::Database;
use crate::{
    DatabaseKey,
    error::{
        BlockStreamError, CompressionConfigError, CryptographyError, DatabaseKeyError, InnerCipherConfigError, KdfConfigError,
        OuterCipherConfigError, VariantDictionaryError, XmlParseError,
    },
    format::{
        DatabaseVersion, DatabaseVersionParseError,
        kdb::KdbOpenError,
        kdb::parse_kdb,
        kdbx3::{Kdbx3OpenError, decrypt_kdbx3, parse_kdbx3},
        kdbx4::{Kdbx4OpenError, decrypt_kdbx4, parse_kdbx4},
    },
};

/// Errors upon reading a database.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseOpenError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Key(#[from] DatabaseKeyError),

    #[error(transparent)]
    DatabaseIntegrity(#[from] DatabaseIntegrityError),

    #[error(transparent)]
    Version(#[from] DatabaseVersionParseError),

    #[error("Opening this database version is not supported")]
    UnsupportedVersion,
}

/// Errors stemming from corrupted databases.
#[derive(Debug, thiserror::Error)]
pub enum DatabaseIntegrityError {
    #[error(transparent)]
    Kdb(#[from] KdbOpenError),

    #[error(transparent)]
    Kdbx3(#[from] Kdbx3OpenError),

    #[error(transparent)]
    Kdbx4(#[from] Kdbx4OpenError),

    #[error(transparent)]
    Version(#[from] DatabaseVersionParseError),

    #[error("Invalid KDBX identifier")]
    InvalidKDBXIdentifier,

    #[error("Invalid KDBX version: {}.{}.{}", version, file_major_version, file_minor_version)]
    InvalidKDBXVersion {
        version: u32,
        file_major_version: u32,
        file_minor_version: u32,
    },

    #[error("Invalid header size: {}", size)]
    InvalidFixedHeader { size: usize },

    #[error("Invalid field length for type {}: {} (expected {})", field_type, field_size, expected_field_size)]
    InvalidKDBFieldLength {
        field_type: u16,
        field_size: u32,
        expected_field_size: u32,
    },

    #[error("Missing group level")]
    MissingKDBGroupLevel,
    #[error("Invalid KDBX header field ID: {}", field_id)]
    InvalidKDBXHeaderFieldID { field_id: u8 },
    #[error("Invalid group level {} (current level {})", group_level, current_level)]
    InvalidKDBGroupLevel { group_level: u16, current_level: u16 },
    #[error("Missing group ID")]
    MissingKDBGroupId,
    #[error("Invalid group ID {}", group_id)]
    InvalidKDBGroupId { group_id: u32 },
    #[error("Invalid group field type: {}", field_type)]
    InvalidKDBGroupFieldType { field_type: u16 },
    #[error("Invalid entry field type: {}", field_type)]
    InvalidKDBEntryFieldType { field_type: u16 },
    #[error("Incomplete group")]
    IncompleteKDBGroup,
    #[error("Incomplete entry")]
    IncompleteKDBEntry,
    #[error("Invalid fixed cipher ID: {}", cid)]
    InvalidFixedCipherID { cid: u32 },
    #[error("Header hash masmatch")]
    HeaderHashMismatch,
    #[error("Invalid outer header entry: {}", entry_type)]
    InvalidOuterHeaderEntry { entry_type: u8 },
    #[error("Incomplete outer header: Missing {}", missing_field)]
    IncompleteOuterHeader { missing_field: String },
    #[error("Invalid inner header entry: {}", entry_type)]
    InvalidInnerHeaderEntry { entry_type: u8 },
    #[error("Incomplete outer header: Missing {}", missing_field)]
    IncompleteInnerHeader { missing_field: String },

    #[error(transparent)]
    Cryptography(#[from] CryptographyError),
    #[error(transparent)]
    Xml(#[from] XmlParseError),
    #[error(transparent)]
    OuterCipher(#[from] OuterCipherConfigError),
    #[error(transparent)]
    InnerCipher(#[from] InnerCipherConfigError),
    #[error(transparent)]
    Compression(#[from] CompressionConfigError),
    #[error(transparent)]
    BlockStream(#[from] BlockStreamError),
    #[error(transparent)]
    VariantDictionary(#[from] VariantDictionaryError),
    #[error(transparent)]
    KdfSettings(#[from] KdfConfigError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Database {
    /// Parse a database from a `std::io::Read`
    pub fn open(source: &mut dyn std::io::Read, key: DatabaseKey) -> Result<Database, DatabaseOpenError> {
        let mut data = Vec::new();
        source.read_to_end(&mut data)?;

        Database::parse(data.as_ref(), key)
    }

    pub fn parse(data: &[u8], key: DatabaseKey) -> Result<Database, DatabaseOpenError> {
        let database_version = DatabaseVersion::parse(data)?;

        match database_version {
            DatabaseVersion::KDB(_) => parse_kdb(data, &key),
            DatabaseVersion::KDB2(_) => Err(DatabaseOpenError::UnsupportedVersion),
            DatabaseVersion::KDB3(_) => parse_kdbx3(data, &key),
            DatabaseVersion::KDB4(_) => parse_kdbx4(data, &key),
        }
    }

    /// Helper function to load a database into its internal XML chunks
    pub fn get_xml(source: &mut dyn std::io::Read, key: DatabaseKey) -> Result<Vec<u8>, DatabaseOpenError> {
        let mut data = Vec::new();
        source.read_to_end(&mut data)?;

        let database_version = DatabaseVersion::parse(data.as_ref())?;

        let data = match database_version {
            DatabaseVersion::KDB(_) => return Err(DatabaseOpenError::UnsupportedVersion),
            DatabaseVersion::KDB2(_) => return Err(DatabaseOpenError::UnsupportedVersion),
            DatabaseVersion::KDB3(_) => decrypt_kdbx3(data.as_ref(), &key)?.2,
            DatabaseVersion::KDB4(_) => decrypt_kdbx4(data.as_ref(), &key)?.3,
        };

        Ok(data)
    }

    /// Get the version of a database without decrypting it
    pub fn get_version(source: &mut dyn std::io::Read) -> Result<DatabaseVersion, DatabaseIntegrityError> {
        let mut data = vec![0; DatabaseVersion::get_version_header_size()];
        _ = source.read(&mut data)?;
        Ok(DatabaseVersion::parse(data.as_ref())?)
    }
}
