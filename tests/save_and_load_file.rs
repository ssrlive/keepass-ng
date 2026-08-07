#[cfg(feature = "save_kdbx4")]
#[test]
fn save_and_load_file_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    use keepass_ng::{
        DatabaseKey,
        db::{Database, Entry, Group, Node, group_add_child, rc_refcell_node, with_node, with_node_mut},
    };
    use std::fs::File;

    let mut db = Database::new(Default::default());
    db.meta.database_name = Some("Demo database".to_string());

    let group_node = rc_refcell_node(Group::new("Demo group"));
    let entry_node = rc_refcell_node(Entry::default());

    with_node_mut::<Entry, _, _>(&entry_node, |entry| {
        entry.set_title(Some("Demo entry"));
        entry.set_username(Some("jdoe"));
        entry.set_password(Some("hunter2"));
    });

    group_add_child(&group_node, entry_node, 0)?;
    group_add_child(&db.root, group_node, 0)?;

    let path = "keepass-ng-save_and_load.kdbx";
    let key = DatabaseKey::new().with_password("demopass");

    db.save(&mut File::create(path)?, key.clone())?;
    let db = Database::open(&mut File::open(path)?, key)?;

    let mut saw_group = false;
    let mut saw_entry = false;

    let children = keepass_ng::db::group_get_children(&db.root).unwrap();
    assert_eq!(children.len(), 1);

    let child_group = &children[0];
    with_node::<Group, _, _>(child_group, |group| {
        saw_group = true;
        assert_eq!(group.get_title().unwrap(), "Demo group");
        let group_children = group.get_children();
        assert_eq!(group_children.len(), 1);
        with_node::<Entry, _, _>(&group_children[0], |entry| {
            saw_entry = true;
            assert_eq!(entry.get_title().unwrap(), "Demo entry");
            assert_eq!(entry.get_username().unwrap(), "jdoe");
            assert_eq!(entry.get_password().unwrap(), "hunter2");
        });
    });

    assert!(saw_group, "Expected to find the saved group");
    assert!(saw_entry, "Expected to find the saved entry");

    let _ = std::fs::remove_file(path);
    Ok(())
}
