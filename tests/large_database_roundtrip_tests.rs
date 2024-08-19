mod large_file_roundtrip_tests {
    use keepass::{
        db::{Database, Entry, Node, NodePtr},
        group_add_child, rc_refcell_node, DatabaseKey, Group, NodeIterator,
    };

    /// This can be tuned based on how "large" we expect databases to realistically be.
    const LARGE_DATABASE_ENTRY_COUNT: usize = 1000; // 100000;

    /// Constants for the test database.
    #[cfg(feature = "save_kdbx4")]
    const TEST_DATABASE_FILE_NAME: &str = "demo.kdbx";
    const TEST_DATABASE_PASSWORD: &str = "demopass";

    /// Writing and reading back a large databack should function as expected.
    /// This tests guards against issues that might affect large databases.
    #[test]
    fn write_and_read_large_database() -> Result<(), Box<dyn std::error::Error>> {
        let mut db = Database::new(Default::default());

        db.meta.database_name = Some("Demo database".to_string());

        for i in 0..LARGE_DATABASE_ENTRY_COUNT {
            let entry = rc_refcell_node!(Entry::default());
            entry.borrow_mut().set_title(Some(&format!("Entry_{i}")));
            if let Some(entry) = entry.borrow_mut().as_any_mut().downcast_mut::<Entry>() {
                entry.set_username(Some(&format!("UserName_{i}")));
                entry.set_password(Some(&format!("Password_{i}")));
            }
            group_add_child(&mut db.root, entry, i)?;
        }

        // Define database key.
        let _key = DatabaseKey::new().with_password(TEST_DATABASE_PASSWORD);
        #[cfg(feature = "save_kdbx4")]
        db.save(&mut std::fs::File::create(TEST_DATABASE_FILE_NAME)?, _key.clone())?;

        // Read the database that was written in the previous block.
        #[cfg(feature = "save_kdbx4")]
        let db = Database::open(&mut std::fs::File::open(TEST_DATABASE_FILE_NAME)?, _key)?;
        // Validate that the data is what we expect.
        let mut entry_counter = 0;
        for node in NodeIterator::new(&db.root) {
            if let Some(group) = node.borrow().as_any().downcast_ref::<Group>() {
                println!("Saw group '{}'", group.get_title().expect("Title should be defined"));
            }
            if let Some(entry) = node.borrow().as_any().downcast_ref::<Entry>() {
                assert_eq!(
                    format!("Entry_{entry_counter}"),
                    entry.get_title().expect("Title should be defined")
                );
                assert_eq!(
                    format!("UserName_{entry_counter}"),
                    entry.get_username().expect("Username should be defined")
                );
                assert_eq!(
                    format!("Password_{entry_counter}"),
                    entry.get_password().expect("Password should be defined")
                );
                entry_counter += 1;
            }
        }
        assert_eq!(entry_counter, LARGE_DATABASE_ENTRY_COUNT);
        Ok(())
    }
}
