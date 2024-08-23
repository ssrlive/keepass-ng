# keepass-ng

[![Crates.io](https://img.shields.io/crates/v/keepass-ng.svg)](https://crates.io/crates/keepass-ng)
[![Documentation](https://docs.rs/keepass-ng/badge.svg)](https://docs.rs/keepass-ng/)
[![Build Status](https://github.com/ssrlive/keepass-ng/actions/workflows/merge.yml/badge.svg?branch=refactor)](https://github.com/ssrlive/keepass-ng/actions/workflows/merge.yml)
[![codecov](https://codecov.io/gh/ssrlive/keepass-ng/branch/refactor/graph/badge.svg)](https://codecov.io/gh/ssrlive/keepass-ng)
[![dependency status](https://deps.rs/repo/github/ssrlive/keepass-ng/status.svg)](https://deps.rs/repo/github/ssrlive/keepass-ng)
[![License file](https://img.shields.io/github/license/ssrlive/keepass-ng)](https://github.com/ssrlive/keepass-ng/blob/refactor/LICENSE)

Rust KeePass database file parser for KDB, KDBX3 and KDBX4, with experimental support for KDBX4 writing.

## Usage
<details>
<summary>

### Open a database
</summary>

```rust
use keepass_ng::{
    db::{node_is_group, Entry, Node, NodeIterator},
    error::DatabaseOpenError,
    Database, DatabaseKey,
};
use std::fs::File;

fn main() -> Result<(), DatabaseOpenError> {
    // Open KeePass database using a password (keyfile is also supported)
    let mut file = File::open("tests/resources/test_db_with_password.kdbx")?;
    let key = DatabaseKey::new().with_password("demopass");
    let db = Database::open(&mut file, key)?;

    // Iterate over all `Group`s and `Entry`s
    for node in NodeIterator::new(&db.root).into_iter() {
        if node_is_group(&node) {
            println!(
                "Saw group '{0}'",
                node.borrow().get_title().unwrap_or("(no title)")
            );
        } else if let Some(e) = node.borrow().as_any().downcast_ref::<Entry>() {
            let title = e.get_title().unwrap_or("(no title)");
            let user = e.get_username().unwrap_or("(no username)");
            let pass = e.get_password().unwrap_or("(no password)");
            println!("Entry '{0}': '{1}' : '{2}'", title, user, pass);
        }
    }

    Ok(())
}
```
</details>

<details>
<summary>

### Save a KDBX4 database (EXPERIMENTAL)

</summary>

**IMPORTANT:** The inner XML data structure will be re-written from scratch from the internal object representation of this crate, so any field that is not parsed by the library will be lost in the written output file! Please make sure to back up your database before trying this feature.

You can enable the experimental support for saving KDBX4 databases using the `save_kdbx4` feature.

```rust
use keepass_ng::{
    db::{group_add_child, Database, Entry, Group, Node, Value},
    rc_refcell_node, DatabaseConfig, DatabaseKey, NodePtr,
};
use std::fs::File;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut db = Database::new(DatabaseConfig::default());

    db.meta.database_name = Some("Demo database".to_string());

    let entry = rc_refcell_node!(Entry::default());
    if let Some(entry) = entry.borrow_mut().as_any_mut().downcast_mut::<Entry>() {
        entry.set_title(Some("Demo entry"));
        entry.set_username(Some("jdoe"));
        entry.set_password(Some("hunter2"));
    }

    let group = rc_refcell_node!(Group::new("Demo group"));
    group_add_child(&group, entry, 0).unwrap();

    group_add_child(&db.root, group, 0).unwrap();

    #[cfg(feature = "save_kdbx4")]
    db.save(&mut File::create("demo.kdbx")?, DatabaseKey::new().with_password("demopass"))?;

    Ok(())
}
```

</details>

<details>
<summary>

### Use developer tools

</summary>

This crate contains several command line tools that can be enabled with the `utilities` feature flag.
See the `[[bin]]` sections in [Cargo.toml](Cargo.toml) for a complete list.

An example command line for running the `kp-dump-xml` command would be:

```bash
cargo run --release --features "utilities" --bin kp-dump-xml -- path/to/database.kdbx
```

</details>


## Installation
Add the following to the `dependencies` section of your `Cargo.toml`:

```toml
[dependencies]
keepass-ng = "*" # TODO replace with current version
```

### Performance Notes

Please set the `RUSTFLAGS` environment variable when compiling to enable CPU-specific optimizations (this greatly affects the speed of the AES key derivation):

```bash
export RUSTFLAGS='-C target-cpu=native'
```

For best results, also compile in Release mode.

Alternatively, you can add a `.cargo/config.toml` like in this project to ensure that rustflags are always set.

#### For AArch64 / ARMv8:

The `aes` optimizations are not yet enabled on stable rust. If you want a big performance boost you can build using nightly and enabling the `armv8` feature of the `aes` crate:

```toml
[dependencies.aes]
# Needs at least 0.7.5 for the feature
version = "0.7.5"
features = ["armv8"]
```

## License
MIT
