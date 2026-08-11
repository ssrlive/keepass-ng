//! Example for merging two versions of the same KeePass database.
//!
//! `Database::merge` performs a two-way, newest-wins merge and returns a
//! [`MergeLog`](keepass_ng::db::merge::MergeLog).
//!
//! Three-way merging
//!
//! This example should be extended to cover it once merged.

#![cfg(feature = "merge")]

use chrono::Duration;
use keepass_ng::DatabaseVersion;
use keepass_ng::db::merge::{MergeError, MergeEvent, MergeEventType, MergeLog};
use keepass_ng::db::{Database, Entry, Node, NodeIterator, with_node, with_node_mut};

#[test]
fn merge_databases_tests() -> Result<(), Box<dyn std::error::Error>> {
    // Start from a shared database and take a second copy to diverge from it.
    let mut destination = Database::new(Default::default());
    assert_eq!(destination.config.version, DatabaseVersion::KDB4(1));
    let root_uuid = destination.root.borrow().get_uuid();
    let shared_entry = destination.create_new_entry(root_uuid, 0)?;
    let shared_entry_id = shared_entry.borrow().get_uuid();
    with_node_mut::<Entry, _, _>(&shared_entry, |entry| {
        entry.set_title(Some("shared entry"));
    })
    .ok_or("shared entry is not an Entry")?;
    let source = destination.clone();

    // Add an entry on the source side that the destination has not seen.
    let source_root_uuid = source.root.borrow().get_uuid();
    let source_entry = source.create_new_entry(source_root_uuid, 1)?;
    with_node_mut::<Entry, _, _>(&source_entry, |entry| {
        entry.set_title(Some("added on source"));
    })
    .ok_or("source entry is not an Entry")?;

    // Edit shared field, but later for destination (destination wins).
    // Times::now() has second precision, so sleep 1 s between the two edits to
    // guarantee destination's timestamp is strictly greater than source's.
    let source_shared_entry = source.search_node_by_uuid(shared_entry_id).ok_or("shared source entry missing")?;
    with_node_mut::<Entry, _, _>(&source_shared_entry, |entry| {
        let current_time = entry.get_times().get_last_modification().unwrap();
        entry.set_username(Some("LosingUser"));
        entry
            .get_times_mut()
            .set_last_modification(Some(current_time + Duration::seconds(1)));
    })
    .ok_or("shared source entry is not an Entry")?;
    std::thread::sleep(std::time::Duration::from_secs(1));
    let destination_shared_entry = destination
        .search_node_by_uuid(shared_entry_id)
        .ok_or("shared destination entry missing")?;
    with_node_mut::<Entry, _, _>(&destination_shared_entry, |entry| {
        let current_time = entry.get_times().get_last_modification().unwrap();
        entry.set_username(Some("WinningUser"));
        entry
            .get_times_mut()
            .set_last_modification(Some(current_time + Duration::seconds(2)));
    })
    .ok_or("shared destination entry is not an Entry")?;

    // Use `MergeLog` / `MergeError` to inspect the merge.
    let log: MergeLog = {
        let result: Result<MergeLog, MergeError> = destination.merge(&source);
        result?
    };

    // Report any issues (e.g. due to lack of tracking)
    if log.warnings.is_empty() {
        println!("Merge completed with no issues.");
    } else {
        println!("Merge completed with {} warning(s):", log.warnings.len());
        for warning in &log.warnings {
            println!("  - {warning}");
        }
    }

    // Categorise each change by matching on the public event enums.
    println!("Applied {} change(s):", log.events.len());
    for MergeEvent { node_uuid, event_type } in &log.events {
        let kind = match event_type {
            MergeEventType::EntryCreated | MergeEventType::GroupCreated | MergeEventType::IconCreated => "created",
            MergeEventType::EntryDeleted | MergeEventType::GroupDeleted => "deleted",
            MergeEventType::EntryLocationUpdated | MergeEventType::GroupLocationUpdated => "moved",
            MergeEventType::EntryUpdated | MergeEventType::GroupUpdated | MergeEventType::IconUpdated => "updated",
        };
        println!("  - {kind} object {node_uuid}");
    }

    // Print each entry in the merged database.
    println!("\nEntries in merged database:");
    for node in NodeIterator::new(&destination.root) {
        with_node::<Entry, _, _>(&node, |entry| {
            println!(
                "  title={:?}  username={:?}",
                entry.get_title().unwrap_or("<no title>"),
                entry.get_username().unwrap_or("<no username>")
            );
        });
    }

    let merged_shared_entry = destination
        .search_node_by_uuid(shared_entry_id)
        .ok_or("merged shared entry missing")?;
    with_node::<Entry, _, _>(&merged_shared_entry, |entry| {
        assert_eq!(entry.get_username(), Some("WinningUser"));
    });
    assert!(destination.search_node_by_uuid(source_entry.borrow().get_uuid()).is_some());

    Ok(())
}
