mod entry_tests {
    use keepass_ng::{
        db::{Database, Entry, Group, Node},
        error::{DatabaseKeyError, DatabaseOpenError},
        DatabaseKey,
    };
    use std::{fs::File, path::Path};
    use uuid::uuid;

    #[test]
    fn kdbx3_entry() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_password.kdbx");
        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        // get an entry on the root node
        if let Some(e) = Group::get(&db.root, &["Sample Entry"]) {
            let e = e.borrow();
            let e = e.as_any().downcast_ref::<Entry>().unwrap();
            assert_eq!(e.get_uuid(), uuid!("0ebeddb2-ed4e-5144-bc34-1a309266a513"));
            assert_eq!(e.get_title(), Some("Sample Entry"));
            assert_eq!(e.get_username(), Some("User Name"));
            assert_eq!(e.get_password(), Some("Password"));
            assert_eq!(e.get_url(), Some("http://keepass.info/"));
            assert_eq!(e.get("custom attribute"), Some("data for custom attribute"));
            assert_eq!(e.get("URL"), Some("http://keepass.info/"));
            assert!(!e.get_times().get_expires());

            let et = chrono::NaiveDateTime::parse_from_str("2016-01-06 09:43:01", "%Y-%m-%d %H:%M:%S").unwrap();
            assert_eq!(e.get_times().get_expiry_time(), Some(et));

            if let Some(at) = e.get_autotype() {
                if let Some(ref s) = at.sequence {
                    assert_eq!(s, "{USERNAME}{TAB}{TAB}{PASSWORD}{ENTER}");
                } else {
                    panic!("Expected a sequence")
                }
            } else {
                panic!("Expected an AutoType entry");
            }
        } else {
            panic!("Expected an entry");
        }

        if let Some(e) = Group::get(&db.root, &["General", "Subgroup", "test entry"]) {
            let e = e.borrow();
            let e = e.as_any().downcast_ref::<Entry>().unwrap();
            assert_eq!(e.get_uuid(), uuid!("5e4c8ad1-9cd5-394c-9039-1178dc140b4a"));
            assert_eq!(e.get_title(), Some("test entry"));
            assert_eq!(e.get_username(), Some("jdoe"));
            assert_eq!(e.get_password(), Some("nWuu5AtqsxqNhnYgLwoB"));
            assert_eq!(e.get_url(), None);
            assert!(!e.get_times().get_expires());
            if let Some(t) = e.get_times().get_expiry_time() {
                assert_eq!(format!("{}", t), "2016-01-28 12:25:36");
            } else {
                panic!("Expected an ExpiryTime");
            }
        } else {
            panic!("Expected an entry");
        }

        Ok(())
    }

    #[test]
    fn kdbx4_entry() -> Result<(), DatabaseOpenError> {
        // KDBX4 database format Base64 encodes ExpiryTime (and all other XML timestamps)
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_aes.kdbx");
        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        // get an entry on the root node
        if let Some(e) = Group::get(&db.root, &["ASDF"]) {
            let e = e.borrow();
            let e = e.as_any().downcast_ref::<Entry>().unwrap();
            assert_eq!(e.get_uuid(), uuid!("4f3816bd83304865879fa108a12f285c"));
            assert_eq!(e.get_title(), Some("ASDF"));
            assert_eq!(e.get_username(), Some("ghj"));
            assert_eq!(e.get_password(), Some("klmno"));
            assert_eq!(e.get_url(), Some("https://example.com"));
            assert_eq!(e.get_tags(), &vec!["keepass-rs".to_string(), "test".to_string()]);
            assert!(e.get_times().get_expires());
            if let Some(t) = e.get_times().get_expiry_time() {
                assert_eq!(format!("{}", t), "2021-04-10 16:53:18");
            } else {
                panic!("Expected an ExpiryTime");
            }
        } else {
            panic!("Expected an entry");
        }

        Ok(())
    }
    #[test]
    fn kdbx4_entry_bad_password() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_aes.kdbx");
        let key = DatabaseKey::new().with_password("this password is not correct");
        let db = Database::open(&mut File::open(path)?, key);

        assert!(db.is_err());

        Ok(())
    }

    #[test]
    fn databasekeyerror_into_databaseopenerror() -> Result<(), DatabaseOpenError> {
        let _: DatabaseOpenError = DatabaseKeyError::IncorrectKey.into();
        Ok(())
    }
}
