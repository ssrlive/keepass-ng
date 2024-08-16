pub(crate) mod kdb;
pub(crate) mod kdbx3;
pub(crate) mod kdbx4;
pub(crate) mod kdbx_header_field_id;

use std::io::Write;

use byteorder::{ByteOrder, LittleEndian, WriteBytesExt};

use crate::error::DatabaseIntegrityError;

const KDBX_IDENTIFIER: [u8; 4] = [0x03, 0xd9, 0xa2, 0x9a];

/// Identifier for `KeePass` 1 format.
pub const KEEPASS_1_ID: u32 = 0xb54b_fb65;
/// Identifier for `KeePass` 2 pre-release format.
pub const KEEPASS_2_ID: u32 = 0xb54b_fb66;
/// Identifier for the latest `KeePass` formats.
pub const KEEPASS_LATEST_ID: u32 = 0xb54b_fb67;

pub const KDBX3_MAJOR_VERSION: u16 = 3;
pub const KDBX4_MAJOR_VERSION: u16 = 4;

pub const KDBX4_CURRENT_MINOR_VERSION: u16 = 0;

/// Supported KDB database versions, with the associated
/// minor version.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub enum DatabaseVersion {
    KDB(u16),
    KDB2(u16),
    KDB3(u16),
    KDB4(u16),
}

impl DatabaseVersion {
    pub fn parse(data: &[u8]) -> Result<DatabaseVersion, DatabaseIntegrityError> {
        // check identifier
        if data.get(0..4) != Some(&KDBX_IDENTIFIER) {
            return Err(DatabaseIntegrityError::InvalidKDBXIdentifier);
        }

        let version = data.get(4..8).map_or(0, LittleEndian::read_u32);
        let file_minor_version = data.get(8..10).map_or(0, LittleEndian::read_u16);
        let file_major_version = data.get(10..12).map_or(0, LittleEndian::read_u16);

        let response = match version {
            KEEPASS_1_ID => DatabaseVersion::KDB(file_minor_version),
            KEEPASS_2_ID => DatabaseVersion::KDB2(file_minor_version),
            KEEPASS_LATEST_ID if file_major_version == KDBX3_MAJOR_VERSION => DatabaseVersion::KDB3(file_minor_version),
            KEEPASS_LATEST_ID if file_major_version == KDBX4_MAJOR_VERSION => DatabaseVersion::KDB4(file_minor_version),
            _ => {
                return Err(DatabaseIntegrityError::InvalidKDBXVersion {
                    version,
                    file_major_version: u32::from(file_major_version),
                    file_minor_version: u32::from(file_minor_version),
                })
            }
        };

        Ok(response)
    }

    fn dump(&self, writer: &mut dyn Write) -> Result<(), std::io::Error> {
        if let DatabaseVersion::KDB4(minor_version) = self {
            _ = writer.write(&crate::format::KDBX_IDENTIFIER)?;
            writer.write_u32::<LittleEndian>(KEEPASS_LATEST_ID)?;
            writer.write_u16::<LittleEndian>(*minor_version)?;
            writer.write_u16::<LittleEndian>(KDBX4_MAJOR_VERSION)?;

            Ok(())
        } else {
            panic!("DatabaseVersion::dump only supports dumping KDBX4.");
        }
    }

    pub(crate) fn get_version_header_size() -> usize {
        12
    }
}

impl std::fmt::Display for DatabaseVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatabaseVersion::KDB(_) => write!(f, "KDB"),
            DatabaseVersion::KDB2(_) => write!(f, "KDBX2"),
            DatabaseVersion::KDB3(minor_version) => write!(f, "KDBX3.{}", minor_version),
            DatabaseVersion::KDB4(minor_version) => write!(f, "KDBX4.{}", minor_version),
        }
    }
}
