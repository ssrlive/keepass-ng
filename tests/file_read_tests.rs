mod file_read_tests {
    #[cfg(feature = "challenge_response")]
    use keepass_ng::ChallengeResponseKey;
    use keepass_ng::{
        db::{Database, Entry, Group, Node, NodeIterator, NodePtr},
        error::{DatabaseIntegrityError, DatabaseOpenError},
        group_get_children, DatabaseKey,
    };
    use std::{fs::File, path::Path};
    use uuid::uuid;

    #[test]
    fn open_kdbx3_with_password() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_password.kdbx");
        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "sample");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 5);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                println!("Saw group '{0}'", g.get_title().unwrap());
                total_groups += 1;
            } else if let Some(e) = node.borrow().as_any().downcast_ref::<Entry>() {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{0}': '{1}' : '{2}'", title, user, pass);
                total_entries += 1;
            }
        }

        assert_eq!(total_groups, 5);
        assert_eq!(total_entries, 6);

        println!("{:?}", db);

        Ok(())
    }

    #[test]
    fn open_kdbx3_with_keyfile() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_keyfile.kdbx");
        let kf_path = Path::new("tests/resources/test_key.key");
        let key = DatabaseKey::new().with_keyfile(&mut File::open(kf_path)?)?;
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                println!("Saw group '{0}'", g.get_title().unwrap());
                total_groups += 1;
            } else if let Some(e) = node.borrow().as_any().downcast_ref::<Entry>() {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{0}': '{1}' : '{2}'", title, user, pass);
                total_entries += 1;
            }
        }

        assert_eq!(total_groups, 1);
        assert_eq!(total_entries, 1);

        println!("{:?}", db);

        Ok(())
    }

    #[test]
    fn open_kdbx3_with_keyfile_xml() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_keyfile_xml.kdbx");
        let kf_path = Path::new("tests/resources/test_key_xml.key");
        let key = DatabaseKey::new().with_keyfile(&mut File::open(kf_path)?)?;
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 4);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                println!("Saw group '{0}'", g.get_title().unwrap());
                total_groups += 1;
            } else if let Some(e) = node.borrow().as_any().downcast_ref::<Entry>() {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{0}': '{1}' : '{2}'", title, user, pass);
                total_entries += 1;
            }
        }

        assert_eq!(total_groups, 5);
        assert_eq!(total_entries, 6);

        println!("{:?}", db);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2_cipher_aes() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 2);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2id_cipher_aes() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2id.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 2);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_aes_cipher_aes() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_aes.kdbx");
        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2_cipher_twofish() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2_twofish.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2_cipher_chacha20() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2_chacha20.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2id_cipher_twofish() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2id_twofish.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2id_cipher_chacha20() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2id_chacha20.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_keyfile() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_keyfile.kdbx");
        let kf_path = Path::new("tests/resources/test_key.key");

        let key = DatabaseKey::new().with_keyfile(&mut File::open(kf_path)?)?;
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_keyfile_v2() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_keyfile_v2.kdbx");
        let kf_path = Path::new("tests/resources/test_db_kdbx4_with_keyfile_v2.keyx");

        let db = Database::open(
            &mut File::open(path)?,
            DatabaseKey::new()
                .with_password("demopass")
                .with_keyfile(&mut File::open(kf_path)?)?,
        )?;

        println!("{:?} DB Opened", db.config);

        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    #[should_panic(expected = r#"InvalidKDBXIdentifier"#)]
    fn open_broken_random_data() {
        let path = Path::new("tests/resources/broken_random_data.kdbx");
        let key = DatabaseKey::new().with_password("");
        Database::open(&mut File::open(path).unwrap(), key).unwrap();
    }

    #[test]
    #[should_panic(expected = r#"InvalidKDBXVersion"#)]
    fn open_broken_kdbx_version() {
        let path = Path::new("tests/resources/broken_kdbx_version.kdbx");
        let key = DatabaseKey::new().with_password("");
        Database::open(&mut File::open(path).unwrap(), key).unwrap();
    }

    #[test]
    fn open_kdb_with_password() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdb_with_password.kdb");
        let key = DatabaseKey::new().with_password("foobar");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 3);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                println!("Saw group '{0}'", g.get_title().unwrap_or("(no title)"));
                total_groups += 1;
            } else if let Some(e) = node.borrow().as_any().downcast_ref::<Entry>() {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{0}': '{1}' : '{2}'", title, user, pass);
                total_entries += 1;
            }
        }

        assert_eq!(total_groups, 12);
        assert_eq!(total_entries, 5);

        println!("{:?}", db);

        Ok(())
    }

    #[test]
    fn open_kdb_with_larger_than_1mb_file_does_not_crash() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdb3_with_file_larger_1mb.kdbx");
        let key = DatabaseKey::new().with_password("samplepassword");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            if let Some(g) = node.borrow().as_any().downcast_ref::<Group>() {
                println!("Saw group '{0}'", g.get_title().unwrap_or("(no title)"));
                total_groups += 1;
            } else if let Some(e) = node.borrow().as_any().downcast_ref::<Entry>() {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{0}': '{1}' : '{2}'", title, user, pass);
                total_entries += 1;
            }
        }

        assert_eq!(total_groups, 1);
        assert_eq!(total_entries, 1);

        println!("{:?}", db);
        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_deleted_entry() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_deleted_entry.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{:?} DB Opened", db);

        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        let recycle_bin_uuid = db.get_recycle_bin().unwrap().borrow().get_uuid();
        assert_eq!(recycle_bin_uuid, uuid!("563171fe-6598-42dc-8003-f98dde32e872"));

        let recycle_group: Vec<NodePtr> = NodeIterator::new(&db.root)
            .filter(|child| {
                if let Some(g) = child.borrow().as_any().downcast_ref::<Group>() {
                    g.get_uuid() == recycle_bin_uuid
                } else {
                    false
                }
            })
            .collect();

        assert_eq!(recycle_group.len(), 1);
        let group = &recycle_group[0];
        if let Some(g) = group.borrow().as_any().downcast_ref::<Group>() {
            assert_eq!(g.get_title().unwrap(), "Recycle Bin");
        } else {
            panic!("It should've matched a Group!");
        }
        Ok(())
    }

    #[test]
    #[cfg(feature = "challenge_response")]
    fn open_kdbx4_with_challenge_response_key() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_challenge_response_key.kdbx");
        let db = Database::open(
            &mut File::open(path)?,
            DatabaseKey::new()
                .with_password("demopass")
                .with_challenge_response_key(ChallengeResponseKey::LocalChallenge(
                    "0102030405060708090a0b0c0d0e0f1011121314".to_string(),
                )),
        )?;

        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 2);
        Ok(())
    }

    #[test]
    #[ignore]
    #[cfg(feature = "challenge_response")]
    fn open_kdbx4_with_yubikey_challenge_response_key() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_challenge_response_key.kdbx");
        let yubikey = ChallengeResponseKey::get_yubikey(None)?;
        let db = Database::open(
            &mut File::open(path)?,
            DatabaseKey::new()
                .with_password("demopass")
                .with_challenge_response_key(ChallengeResponseKey::YubikeyChallenge(yubikey, "2".to_string())),
        )?;

        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 2);
        Ok(())
    }

    #[test]
    fn test_get_version() -> Result<(), DatabaseIntegrityError> {
        let path = Path::new("tests/resources/test_db_with_password.kdbx");
        let version = Database::get_version(&mut File::open(path)?)?;
        assert_eq!(version.to_string(), "KDBX3.1");

        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2.kdbx");
        let version = Database::get_version(&mut File::open(path)?)?;
        assert_eq!(version.to_string(), "KDBX4.0");

        Ok(())
    }
}
