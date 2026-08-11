//! Types for representing data contained in a `KeePass` database

mod types;

mod open;

pub use crate::db::open::{DatabaseIntegrityError, DatabaseOpenError};

#[cfg(feature = "merge")]
pub mod merge;

#[cfg(feature = "totp")]
pub(crate) mod otp;

#[cfg(feature = "save_kdbx4")]
mod save;

#[cfg(feature = "save_kdbx4")]
pub use crate::db::save::DatabaseSaveError;

pub use crate::db::types::*;
pub use crate::key::{DatabaseKey, DatabaseKeyError};

#[cfg(feature = "totp")]
pub use crate::db::otp::{TOTP, TOTPAlgorithm, TOTPError};

#[cfg(test)]
mod database_tests {
    use crate::{
        Result,
        db::{Database, DatabaseKey},
    };
    #[cfg(feature = "save_kdbx4")]
    use crate::{config::DatabaseConfig, db::Entry};
    use std::fs::File;

    #[test]
    fn test_xml() -> Result<()> {
        let key = DatabaseKey::new().with_password("demopass");
        let mut f = File::open("tests/resources/test_db_with_password.kdbx")?;
        let xml = Database::get_xml(&mut f, key)?;

        assert!(xml.len() > 100);

        Ok(())
    }

    #[test]
    fn test_open_invalid_version_header_size() {
        assert!(Database::parse(&[], DatabaseKey::new().with_password("testing")).is_err());
        assert!(Database::parse(&[0, 0, 0, 0, 0, 0, 0, 0], DatabaseKey::new().with_password("testing")).is_err());
        assert!(Database::parse(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], DatabaseKey::new().with_password("testing")).is_err());
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn test_save() -> Result<()> {
        use crate::{
            db::{Group, group_add_child, rc_refcell_node},
            format::variant_dictionary::VariantDictionary,
        };

        let mut db = Database::new(DatabaseConfig::default());

        let mut public_custom_data = VariantDictionary::new();
        public_custom_data.set("example", 42);

        db.config.public_custom_data = Some(public_custom_data);

        group_add_child(&db.root, rc_refcell_node(Entry::default()), 0).unwrap();
        group_add_child(&db.root, rc_refcell_node(Entry::default()), 1).unwrap();
        group_add_child(&db.root, rc_refcell_node(Entry::default()), 2).unwrap();

        let group = rc_refcell_node(Group::new("my group"));
        group_add_child(&group, rc_refcell_node(Entry::default()), 0).unwrap();
        group_add_child(&group, rc_refcell_node(Entry::default()), 1).unwrap();
        group_add_child(&db.root, group, 3).unwrap();

        let mut buffer = Vec::new();
        let key = DatabaseKey::new().with_password("testing");

        db.save(&mut buffer, key.clone())?;

        let db_loaded = Database::open(&mut buffer.as_slice(), key)?;

        assert_eq!(db, db_loaded);
        Ok(())
    }
}
