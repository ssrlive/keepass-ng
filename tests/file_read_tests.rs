mod file_read_tests {
    #[cfg(feature = "challenge_response")]
    use keepass_ng::ChallengeResponseKey;
    use keepass_ng::{
        DatabaseIntegrityError, DatabaseKey, DatabaseOpenError, DatabaseVersion,
        db::{CustomDataValue, Database, Entry, Group, Node, NodeIterator, NodePtr, group_get_children, with_node},
    };
    use std::{fs::File, path::Path};
    use uuid::uuid;

    #[test]
    fn open_kdbx3_with_password() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_password.kdbx");
        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB3(1));
        assert_eq!(db.root.borrow().get_title().unwrap(), "sample");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 5);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            with_node::<Group, _, _>(&node, |g| {
                println!("Saw group '{0}'", g.get_title().unwrap());
                total_groups += 1;
            });
            with_node::<Entry, _, _>(&node, |e| {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{title}': '{user}' : '{pass}'");
                total_entries += 1;
            });
        }

        assert_eq!(total_groups, 5);
        assert_eq!(total_entries, 6);

        println!("{db:?}");

        Ok(())
    }

    #[test]
    fn open_kdbx3_with_keyfile() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_keyfile.kdbx");
        let kf_path = Path::new("tests/resources/test_key.key");
        let key = DatabaseKey::new().with_keyfile(&mut File::open(kf_path)?)?;
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB3(1));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            with_node::<Group, _, _>(&node, |g| {
                println!("Saw group '{0}'", g.get_title().unwrap());
                total_groups += 1;
            });
            with_node::<Entry, _, _>(&node, |e| {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{title}': '{user}' : '{pass}'");
                total_entries += 1;
            });
        }

        assert_eq!(total_groups, 1);
        assert_eq!(total_entries, 1);

        println!("{db:?}");

        Ok(())
    }

    #[test]
    fn open_kdbx3_with_keyfile_xml() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_with_keyfile_xml.kdbx");
        let kf_path = Path::new("tests/resources/test_key_xml.key");
        let key = DatabaseKey::new().with_keyfile(&mut File::open(kf_path)?)?;
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB3(1));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 4);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            with_node::<Group, _, _>(&node, |g| {
                println!("Saw group '{0}'", g.get_title().unwrap());
                total_groups += 1;
            });
            with_node::<Entry, _, _>(&node, |e| {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{title}': '{user}' : '{pass}'");
                total_entries += 1;
            });
        }

        assert_eq!(total_groups, 5);
        assert_eq!(total_entries, 6);

        println!("{db:?}");

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2_cipher_aes() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 2);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2id_cipher_aes() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2id.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 2);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_aes_cipher_aes() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_aes.kdbx");
        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB4(1));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2_cipher_twofish() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2_twofish.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2_cipher_chacha20() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2_chacha20.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2id_cipher_twofish() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2id_twofish.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_kdf_argon2id_cipher_chacha20() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_argon2id_chacha20.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));
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

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));
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
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));

        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        Ok(())
    }

    /// keyfile with tabs in the keyfile content (#284)
    #[test]
    fn open_kdbx4_with_keyfile_v2_alt() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_keyfile_v2_alt.kdbx");
        let kf_path = Path::new("tests/resources/test_db_kdbx4_with_keyfile_v2_alt.keyx");

        let db = Database::open(
            &mut File::open(path)?,
            DatabaseKey::new()
                .with_password("demopass")
                .with_keyfile(&mut File::open(kf_path)?)?,
        )?;

        println!("{:?} DB Opened", db);
        assert_eq!(db.config.version, DatabaseVersion::KDB4(0));
        assert_eq!(db.root.borrow().get_title().unwrap(), "testdb02");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 8);

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

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB(2));
        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        assert_eq!(group_get_children(&db.root).unwrap().len(), 3);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            with_node::<Group, _, _>(&node, |g| {
                println!("Saw group '{0}'", g.get_title().unwrap_or("(no title)"));
                total_groups += 1;
            });
            with_node::<Entry, _, _>(&node, |e| {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{title}': '{user}' : '{pass}'");
                total_entries += 1;
            });
        }

        assert_eq!(total_groups, 12);
        assert_eq!(total_entries, 5);

        println!("{db:?}");

        Ok(())
    }

    #[test]
    fn open_kdb_with_larger_than_1mb_file_does_not_crash() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdb3_with_file_larger_1mb.kdbx");
        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");
        assert_eq!(db.config.version, DatabaseVersion::KDB3(1));
        assert_eq!(group_get_children(&db.root).unwrap().len(), 1);

        let mut total_groups = 0;
        let mut total_entries = 0;
        for node in NodeIterator::new(&db.root) {
            with_node::<Group, _, _>(&node, |g| {
                println!("Saw group '{0}'", g.get_title().unwrap_or("(no title)"));
                total_groups += 1;
            });
            with_node::<Entry, _, _>(&node, |e| {
                let title = e.get_title().unwrap_or("(no title)");
                let user = e.get_username().unwrap_or("(no user)");
                let pass = e.get_password().unwrap_or("(no password)");
                println!("Entry '{title}': '{user}' : '{pass}'");
                total_entries += 1;
            });
        }

        assert_eq!(total_groups, 1);
        assert_eq!(total_entries, 1);

        println!("{db:?}");
        Ok(())
    }

    #[test]
    fn open_kdbx4_with_password_deleted_entry() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_password_deleted_entry.kdbx");

        let key = DatabaseKey::new().with_password("demopass");
        let db = Database::open(&mut File::open(path)?, key)?;

        println!("{db:?} DB Opened");

        assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
        let recycle_bin_uuid = db.get_recycle_bin().unwrap().borrow().get_uuid();
        assert_eq!(recycle_bin_uuid, uuid!("563171fe-6598-42dc-8003-f98dde32e872"));

        let recycle_group: Vec<NodePtr> = NodeIterator::new(&db.root)
            .filter(|child| with_node::<Group, _, _>(child, |g| g.get_uuid() == recycle_bin_uuid).unwrap_or(false))
            .collect();

        assert_eq!(recycle_group.len(), 1);
        let group = &recycle_group[0];
        with_node::<Group, _, _>(group, |g| {
            assert_eq!(g.get_title().unwrap(), "Recycle Bin");
        })
        .unwrap();
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
    fn open_kdbx4_with_empty_root_group_name() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_empty_root_group_name.kdbx");

        let db = Database::open(&mut File::open(path)?, DatabaseKey::new().with_password("demopass"))?;

        println!("{db:?} DB Opened");

        assert_eq!(db.root.borrow().get_title(), None);

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
    fn open_kdbx4_with_empty_group_names() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx4_with_empty_group_names.kdbx");

        let db = Database::open(&mut File::open(path)?, DatabaseKey::new().with_password("demopass"))?;

        println!("{db:?} DB Opened");

        assert_eq!(db.root.borrow().get_title(), Some("Root"));
        assert_eq!(group_get_children(&db.root).unwrap().len(), 2);

        Ok(())
    }

    #[test]
    fn open_kdbx41_with_password() -> Result<(), DatabaseOpenError> {
        let path = Path::new("tests/resources/test_db_kdbx41_with_password_aes.kdbx");
        let db = Database::open(&mut File::open(path)?, DatabaseKey::new().with_password("demopass"))?;

        assert_eq!(db.config.version, DatabaseVersion::KDB4(1));
        assert_eq!(db.root.borrow().get_title(), Some("Database"));

        assert!(
            db.meta
                .custom_data
                .get("KeePassRPC.Config")
                .unwrap()
                .last_modification_time
                .is_some()
        );

        let root: NodePtr = db.root.clone().into();
        let root_group = root.borrow();
        let root_group = root_group.downcast_ref::<Group>().unwrap();
        assert_eq!(root_group.groups().len(), 2);
        assert_eq!(root_group.entries().len(), 4);

        let tagged_group = Group::get(&db.root.clone().into(), &["Group with tags"]).unwrap();
        let tagged_group_id = tagged_group.borrow().get_uuid();
        with_node::<Group, _, _>(&tagged_group, |group| {
            assert_eq!(group.tags(), &["a", "b", "c"]);
            assert!(group.previous_parent_group().is_none());
        });

        let no_quality_check = Group::get(&db.root.clone().into(), &["entry with no quality check"]).unwrap();
        with_node::<Entry, _, _>(&no_quality_check, |entry| assert!(!entry.quality_check()));

        let named_icon_entry = Group::get(&db.root.clone().into(), &["entry with named custom icon"]).unwrap();
        with_node::<Entry, _, _>(&named_icon_entry, |entry| {
            assert!(entry.quality_check());
            let icon_uuid = entry.custom_icon_uuid().unwrap();
            let icon = db.meta.custom_icon(icon_uuid).unwrap();
            assert_eq!(icon.name.as_deref(), Some("Egg"));
            assert!(icon.last_modification_time.is_some());
        });

        let custom_data_entry = Group::get(&db.root.clone().into(), &["entry with custom data"]).unwrap();
        with_node::<Entry, _, _>(&custom_data_entry, |entry| {
            let custom_data = entry.custom_data().get("KPRPC JSON").unwrap();
            let value = custom_data.value.as_ref().unwrap();
            assert_eq!(
                value,
                &CustomDataValue::String(
                    r#"{"version":2,"altUrls":["https://example.com","http://example.com"],"blockedUrls":[],"regExBlockedUrls":[],"regExUrls":[],"authenticationMethods":["password"],"matcherConfigs":[{"matcherType":"Url","urlMatchMethod":"Domain"}],"fields":[{"uuid":"nkd4kOJMJ0qPOJaAjoTXcw==","valuePath":"UserName","page":1,"type":"Text","matcherConfigs":[{"matcherType":"UsernameDefaultHeuristic"}]},{"uuid":"7xtQBZ2+wEizAxNe5rYKzg==","valuePath":"Password","page":1,"type":"Password","matcherConfigs":[{"matcherType":"PasswordDefaultHeuristic"}]}]}"#.to_string()
                )
            );
        });

        let moved_entry = Group::get(&db.root.clone().into(), &["entry that was moved"]).unwrap();
        with_node::<Entry, _, _>(&moved_entry, |entry| {
            assert_eq!(entry.previous_parent_group(), Some(tagged_group_id))
        });

        let moved_group = Group::get(&db.root.clone().into(), &["Group that was moved"]).unwrap();
        with_node::<Group, _, _>(&moved_group, |group| {
            assert_eq!(group.previous_parent_group(), Some(tagged_group_id))
        });

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

        let path = Path::new("tests/resources/test_db_kdbx41_with_password_aes.kdbx");
        let version = Database::get_version(&mut File::open(path)?)?;
        assert_eq!(version.to_string(), "KDBX4.1");

        Ok(())
    }

    #[test]
    fn open_kdbx4_fuzzing() -> Result<(), DatabaseOpenError> {
        use std::io::Read;
        let path = Path::new("tests/resources/test_db_few_rounds.kdbx");
        let file_len = path.metadata()?.len() as usize;

        let mut file_as_vec = Vec::new();
        File::open(path)?.read_to_end(&mut file_as_vec)?;

        for one_len in 0..=file_len {
            print!("Trying length: {}", one_len);
            let current_slice = &file_as_vec[..one_len];
            let res = Database::parse(current_slice, DatabaseKey::new().with_password("demopass"));
            match res {
                Ok(db) => {
                    println!(" - DB Opened");

                    assert_eq!(db.root.borrow().get_title().unwrap(), "Root");
                    assert_eq!(group_get_children(&db.root).unwrap().len(), 1);
                }
                Err(_) => {
                    println!(" - failed with error");
                    continue;
                    // we don't care about error, it's normal, we just check for panic
                }
            }
        }

        Ok(())
    }
}
