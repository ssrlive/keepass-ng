#[cfg(feature = "totp")]
use crate::db::otp::{TOTPError, TOTP};
use crate::{
    db::{
        group::MergeLog,
        node::{Node, NodePtr},
        Color, CustomData, IconId, Times,
    },
    rc_refcell_node,
};
use chrono::NaiveDateTime;
use secstr::SecStr;
use std::{collections::HashMap, thread, time};
use uuid::Uuid;

/// A database entry containing several key-value fields.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Entry {
    pub(crate) uuid: Uuid,
    pub(crate) fields: HashMap<String, Value>,
    pub(crate) autotype: Option<AutoType>,
    pub(crate) tags: Vec<String>,

    pub(crate) times: Times,

    pub(crate) custom_data: CustomData,

    pub(crate) icon_id: Option<IconId>,
    pub(crate) custom_icon_uuid: Option<Uuid>,

    pub(crate) foreground_color: Option<Color>,
    pub(crate) background_color: Option<Color>,

    pub(crate) override_url: Option<String>,
    pub(crate) quality_check: Option<bool>,

    pub(crate) history: Option<History>,

    pub(crate) parent: Option<Uuid>,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            fields: HashMap::new(),
            autotype: None,
            tags: Vec::new(),
            times: Times::new(),
            custom_data: CustomData::default(),
            icon_id: Some(IconId::KEY),
            custom_icon_uuid: None,
            foreground_color: None,
            background_color: None,
            override_url: None,
            quality_check: None,
            history: None,
            parent: None,
        }
    }
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
            && self.fields == other.fields
            && self.autotype == other.autotype
            && self.tags == other.tags
            && self.times == other.times
            && self.custom_data == other.custom_data
            && self.icon_id == other.icon_id
            && self.custom_icon_uuid == other.custom_icon_uuid
            && self.foreground_color == other.foreground_color
            && self.background_color == other.background_color
            && self.override_url == other.override_url
            && self.quality_check == other.quality_check
            && self.history == other.history
        // && self.parent == other.parent
    }
}

impl Eq for Entry {}

impl Node for Entry {
    fn duplicate(&self) -> NodePtr {
        let mut tmp = self.clone();
        tmp.parent = None;
        rc_refcell_node!(tmp)
    }

    fn get_uuid(&self) -> Uuid {
        self.uuid
    }

    fn set_uuid(&mut self, uuid: Uuid) {
        self.uuid = uuid;
    }

    fn get_title(&self) -> Option<&str> {
        self.get("Title")
    }

    fn set_title(&mut self, title: Option<&str>) {
        self.set_unprotected_field_pair("Title", title);
    }

    fn get_notes(&self) -> Option<&str> {
        self.get("Notes")
    }

    fn set_notes(&mut self, notes: Option<&str>) {
        self.set_unprotected_field_pair("Notes", notes);
    }

    fn get_icon_id(&self) -> Option<IconId> {
        self.icon_id
    }

    fn set_icon_id(&mut self, icon_id: Option<IconId>) {
        self.icon_id = icon_id;
    }

    fn get_custom_icon_uuid(&self) -> Option<Uuid> {
        self.custom_icon_uuid
    }

    fn get_times(&self) -> &Times {
        &self.times
    }

    fn get_times_mut(&mut self) -> &mut Times {
        &mut self.times
    }

    fn get_parent(&self) -> Option<Uuid> {
        self.parent
    }

    fn set_parent(&mut self, parent: Option<Uuid>) {
        self.parent = parent;
    }
}

#[allow(dead_code)]
pub fn entry_set_field_and_commit(entry: &NodePtr, field_name: &str, field_value: &str) -> crate::Result<()> {
    entry
        .borrow_mut()
        .as_any_mut()
        .downcast_mut::<Entry>()
        .ok_or("node is not an Entry.")?
        .set_field_and_commit(field_name, field_value);
    Ok(())
}

impl Entry {
    pub fn get_history(&self) -> &Option<History> {
        &self.history
    }

    pub fn purge_history(&mut self) {
        self.history = None;
    }

    pub(crate) fn merge(entry: &NodePtr, other: &NodePtr) -> Result<(NodePtr, MergeLog), String> {
        let mut log = MergeLog::default();

        let mut source_history = match &other.borrow().as_any().downcast_ref::<Entry>().ok_or("Error")?.history {
            Some(h) => h.clone(),
            None => {
                log.warnings.push(format!("Entry {} had no history.", entry.borrow().get_uuid()));
                History::default()
            }
        };
        let mut destination_history = match &entry.borrow().as_any().downcast_ref::<Entry>().ok_or("Error")?.history {
            Some(h) => h.clone(),
            None => {
                log.warnings.push(format!("Entry {} had no history.", entry.borrow().get_uuid()));
                History::default()
            }
        };

        let other = other.borrow().duplicate();
        source_history.add_entry(other.borrow().as_any().downcast_ref::<Entry>().ok_or("Error")?.clone());
        let history_merge_log = destination_history.merge_with(&source_history)?;
        let response = entry.borrow().duplicate();
        response.borrow_mut().as_any_mut().downcast_mut::<Entry>().ok_or("Error")?.history = Some(destination_history);

        Ok((response, log.merge_with(&history_merge_log)))
    }

    // Convenience function used in unit tests, to make sure that:
    // 1. The history gets updated after changing a field
    // 2. We wait a second before commiting the changes so that the timestamp is not the same
    //    as it previously was. This is necessary since the timestamps in the KDBX format
    //    do not preserve the msecs.
    pub(crate) fn set_field_and_commit(&mut self, field_name: &str, field_value: &str) {
        self.set_unprotected_field_pair(field_name, Some(field_value));
        thread::sleep(time::Duration::from_secs(1));
        self.update_history();
    }

    fn set_unprotected_field_pair(&mut self, field_name: &str, field_value: Option<&str>) {
        if let Some(field_value) = field_value {
            self.fields
                .insert(field_name.to_string(), Value::Unprotected(field_value.to_string()));
        } else {
            self.fields.remove(field_name);
        }
    }

    pub(crate) fn entry_replaced_with(entry: &NodePtr, other: &NodePtr) -> Option<()> {
        let mut success = false;
        if let Some(entry) = entry.borrow_mut().as_any_mut().downcast_mut::<Entry>() {
            if let Some(other) = other.borrow().as_any().downcast_ref::<Entry>() {
                entry.uuid = other.uuid;
                entry.fields = other.fields.clone();
                entry.autotype = other.autotype.clone();
                entry.tags = other.tags.clone();
                entry.times = other.times.clone();
                entry.custom_data = other.custom_data.clone();
                entry.icon_id = other.icon_id;
                entry.custom_icon_uuid = other.custom_icon_uuid;
                entry.foreground_color = other.foreground_color;
                entry.background_color = other.background_color;
                entry.override_url = other.override_url.clone();
                entry.quality_check = other.quality_check;
                entry.history = other.history.clone();
                // entry.parent = other.parent;
                success = true;
            }
        }
        if !success {
            return None;
        }
        Some(())
    }
}

impl<'a> Entry {
    /// Get a field by name, taking care of unprotecting Protected values automatically
    pub fn get(&'a self, key: &str) -> Option<&'a str> {
        match self.fields.get(key) {
            None | Some(&Value::Bytes(_)) => None,
            Some(Value::Protected(pv)) => std::str::from_utf8(pv.unsecure()).ok(),
            Some(Value::Unprotected(uv)) => Some(uv),
        }
    }

    /// Get a bytes field by name
    pub fn get_bytes(&'a self, key: &str) -> Option<&'a [u8]> {
        match self.fields.get(key) {
            Some(Value::Bytes(b)) => Some(b),
            _ => None,
        }
    }

    /// Convenience method for getting a TOTP from this entry
    #[cfg(feature = "totp")]
    pub fn get_otp(&'a self) -> Result<TOTP, TOTPError> {
        self.get_raw_otp_value().ok_or(TOTPError::NoRecord)?.parse()
    }

    /// Convenience method for getting the raw value of the 'otp' field
    pub fn get_raw_otp_value(&'a self) -> Option<&'a str> {
        self.get("otp")
    }

    pub fn get_autotype(&self) -> Option<&AutoType> {
        self.autotype.as_ref()
    }

    pub fn set_autotype(&mut self, autotype: Option<AutoType>) {
        self.autotype = autotype;
    }

    /// Convenience method for getting tags
    /// Returns a Vec of tags
    pub fn get_tags(&self) -> &Vec<String> {
        self.tags.as_ref()
    }

    pub fn get_tags_mut(&mut self) -> &mut Vec<String> {
        self.tags.as_mut()
    }

    /// Convenience method for getting the value of the `UserName` field
    pub fn get_username(&'a self) -> Option<&'a str> {
        self.get("UserName")
    }

    pub fn set_username(&mut self, username: Option<&str>) {
        self.set_unprotected_field_pair("UserName", username);
    }

    /// Convenience method for getting the value of the 'Password' field
    pub fn get_password(&self) -> Option<&str> {
        self.get("Password")
    }

    pub fn set_password(&mut self, password: Option<&str>) {
        if let Some(password) = password {
            self.fields
                .insert("Password".to_string(), Value::Protected(password.as_bytes().into()));
        } else {
            self.fields.remove("Password");
        }
    }

    /// Convenience method for getting the value of the 'URL' field
    pub fn get_url(&self) -> Option<&str> {
        self.get("URL")
    }

    pub fn set_url(&mut self, url: Option<&str>) {
        self.set_unprotected_field_pair("URL", url);
    }

    /// Adds the current version of the entry to the entry's history
    /// and updates the last modification timestamp.
    /// The history will only be updated if the entry has
    /// uncommited changes.
    ///
    /// Returns whether or not a new history entry was added.
    pub fn update_history(&mut self) -> bool {
        if self.history.is_none() {
            self.history = Some(History::default());
        }

        if !self.has_uncommited_changes() {
            return false;
        }

        self.times.set_last_modification(Some(Times::now()));

        let mut new_history_entry = self.clone();
        new_history_entry.history = None;

        // TODO should we validate that the history is enabled?
        // TODO should we validate the maximum size of the history?
        if let Some(h) = self.history.as_mut() {
            h.add_entry(new_history_entry);
        }

        true
    }

    /// Determines if the entry was modified since the last
    /// history update.
    fn has_uncommited_changes(&self) -> bool {
        if let Some(history) = self.history.as_ref() {
            if history.entries.is_empty() {
                return true;
            }

            let mut sanitized_entry = self.clone();
            sanitized_entry.times.set_last_modification(Some(NaiveDateTime::default()));
            sanitized_entry.history.take();

            let mut last_history_entry = history.entries.first().unwrap().clone();
            last_history_entry.times.set_last_modification(Some(NaiveDateTime::default()));
            last_history_entry.history.take();

            if sanitized_entry.eq(&last_history_entry) {
                return false;
            }
        }
        true
    }
}

/// A value that can be a raw string, byte array, or protected memory region
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Value {
    Bytes(Vec<u8>),
    Unprotected(String),
    Protected(SecStr),
}

impl Value {
    pub fn is_empty(&self) -> bool {
        match self {
            Value::Bytes(b) => b.is_empty(),
            Value::Unprotected(u) => u.is_empty(),
            Value::Protected(p) => p.unsecure().is_empty(),
        }
    }
}

#[cfg(feature = "serialization")]
impl serde::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Bytes(b) => serializer.serialize_bytes(b),
            Value::Unprotected(u) => serializer.serialize_str(u),
            Value::Protected(p) => serializer.serialize_str(String::from_utf8_lossy(p.unsecure()).as_ref()),
        }
    }
}

/// An `AutoType` setting associated with an Entry
#[derive(Debug, Default, Eq, PartialEq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct AutoType {
    pub enabled: bool,
    pub sequence: Option<String>,
    pub associations: Vec<AutoTypeAssociation>,
}

/// A window association associated with an `AutoType` setting
#[derive(Debug, Default, Eq, PartialEq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct AutoTypeAssociation {
    pub window: Option<String>,
    pub sequence: Option<String>,
}

/// An entry's history
#[derive(Debug, Default, Eq, PartialEq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct History {
    pub(crate) entries: Vec<Entry>,
}
impl History {
    pub fn add_entry(&mut self, mut entry: Entry) {
        // DISCUSS: should we make sure that the last modification time is not the same
        // or older than the entry at the top of the history?
        if entry.history.is_some() {
            // Remove the history from the new history entry to avoid having
            // an exponential number of history entries.
            entry.history = None;
        }
        self.entries.insert(0, entry);
    }

    pub fn get_entries(&self) -> &Vec<Entry> {
        &self.entries
    }

    // Determines if the entries of the history are
    // ordered by last modification time.
    pub(crate) fn is_ordered(&self) -> bool {
        let mut last_modification_time: Option<NaiveDateTime> = None;
        for entry in &self.entries {
            if last_modification_time.is_none() {
                last_modification_time = entry.times.get_last_modification();
            }

            let entry_modification_time = entry.times.get_last_modification().unwrap();
            // FIXME should we also handle equal modification times??
            if last_modification_time.unwrap() < entry_modification_time {
                return false;
            }
            last_modification_time = Some(entry_modification_time);
        }
        true
    }

    // Merge both histories together.
    pub(crate) fn merge_with(&mut self, other: &History) -> Result<MergeLog, String> {
        let mut log = MergeLog::default();
        let mut new_history_entries: HashMap<NaiveDateTime, Entry> = HashMap::new();

        for history_entry in &self.entries {
            let modification_time = history_entry.times.get_last_modification().unwrap();
            if new_history_entries.contains_key(&modification_time) {
                return Err("This should never happen.".to_string());
            }
            new_history_entries.insert(modification_time, history_entry.clone());
        }

        for history_entry in &other.entries {
            let modification_time = history_entry.times.get_last_modification().unwrap();
            let existing_history_entry = new_history_entries.get(&modification_time);
            if let Some(existing_history_entry) = existing_history_entry {
                if !existing_history_entry.eq(history_entry) {
                    log.warnings
                        .push("History entries have the same modification timestamp but were not the same.".to_string());
                }
            } else {
                new_history_entries.insert(modification_time, history_entry.clone());
            }
        }

        let mut all_modification_times: Vec<&NaiveDateTime> = new_history_entries.keys().collect();
        all_modification_times.sort();
        all_modification_times.reverse();
        let mut new_entries: Vec<Entry> = vec![];
        for modification_time in &all_modification_times {
            new_entries.push(new_history_entries.get(modification_time).unwrap().clone());
        }

        self.entries = new_entries;
        if !self.is_ordered() {
            // TODO this should be unit tested.
            return Err("The resulting history is not ordered.".to_string());
        }

        Ok(log)
    }
}

#[cfg(test)]
mod entry_tests {
    use super::{Entry, Node, Value};
    use secstr::SecStr;
    use std::{thread, time};

    #[test]
    fn byte_values() {
        let mut entry = Entry::default();
        entry.fields.insert("a-bytes".to_string(), Value::Bytes(vec![1, 2, 3]));

        entry
            .fields
            .insert("a-unprotected".to_string(), Value::Unprotected("asdf".to_string()));

        entry
            .fields
            .insert("a-protected".to_string(), Value::Protected(SecStr::new("asdf".as_bytes().to_vec())));

        assert_eq!(entry.get_bytes("a-bytes"), Some(&[1, 2, 3][..]));
        assert_eq!(entry.get_bytes("a-unprotected"), None);
        assert_eq!(entry.get_bytes("a-protected"), None);

        assert_eq!(entry.get("a-bytes"), None);

        assert!(!entry.fields["a-bytes"].is_empty());
    }

    #[test]
    fn update_history() {
        let mut entry = Entry::default();
        let mut last_modification_time = entry.times.get_last_modification().unwrap();

        entry.fields.insert("Username".to_string(), Value::Unprotected("user".to_string()));
        // Making sure to wait 1 sec before update the history, to make
        // sure that we get a different modification timestamp.
        thread::sleep(time::Duration::from_secs(1));

        assert!(entry.update_history());
        assert!(entry.history.is_some());
        assert_eq!(entry.history.as_ref().unwrap().entries.len(), 1);
        assert_ne!(entry.times.get_last_modification().unwrap(), last_modification_time);
        last_modification_time = entry.times.get_last_modification().unwrap();
        thread::sleep(time::Duration::from_secs(1));

        // Updating the history without making any changes
        // should not do anything.
        assert!(!entry.update_history());
        assert!(entry.history.is_some());
        assert_eq!(entry.history.as_ref().unwrap().entries.len(), 1);
        assert_eq!(entry.times.get_last_modification().unwrap(), last_modification_time);

        entry.set_title(Some("first title"));

        assert!(entry.update_history());
        assert!(entry.history.is_some());
        assert_eq!(entry.history.as_ref().unwrap().entries.len(), 2);
        assert_ne!(entry.times.get_last_modification().unwrap(), last_modification_time);
        last_modification_time = entry.times.get_last_modification().unwrap();
        thread::sleep(time::Duration::from_secs(1));

        assert!(!entry.update_history());
        assert!(entry.history.is_some());
        assert_eq!(entry.history.as_ref().unwrap().entries.len(), 2);
        assert_eq!(entry.times.get_last_modification().unwrap(), last_modification_time);

        entry.set_title(Some("second title"));

        assert!(entry.update_history());
        assert!(entry.history.is_some());
        assert_eq!(entry.history.as_ref().unwrap().entries.len(), 3);
        assert_ne!(entry.times.get_last_modification().unwrap(), last_modification_time);
        last_modification_time = entry.times.get_last_modification().unwrap();
        thread::sleep(time::Duration::from_secs(1));

        assert!(!entry.update_history());
        assert!(entry.history.is_some());
        assert_eq!(entry.history.as_ref().unwrap().entries.len(), 3);
        assert_eq!(entry.times.get_last_modification().unwrap(), last_modification_time);

        let last_history_entry = entry.history.as_ref().unwrap().entries.first().unwrap();
        assert_eq!(last_history_entry.get_title().unwrap(), "second title");

        for history_entry in &entry.history.unwrap().entries {
            assert!(history_entry.history.is_none());
        }
    }

    #[cfg(feature = "totp")]
    #[test]
    fn totp() {
        let mut entry = Entry::default();
        entry.fields.insert(
            "otp".to_string(),
            Value::Unprotected(
                "otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA1&digits=6&period=30"
                    .to_string(),
            ),
        );

        assert!(entry.get_otp().is_ok());
    }

    #[cfg(feature = "serialization")]
    #[test]
    fn serialization() {
        assert_eq!(
            serde_json::to_string(&Value::Bytes(vec![65, 66, 67])).unwrap(),
            "[65,66,67]".to_string()
        );

        assert_eq!(
            serde_json::to_string(&Value::Unprotected("ABC".to_string())).unwrap(),
            "\"ABC\"".to_string()
        );

        assert_eq!(
            serde_json::to_string(&Value::Protected(SecStr::new("ABC".as_bytes().to_vec()))).unwrap(),
            "\"ABC\"".to_string()
        );
    }
}
