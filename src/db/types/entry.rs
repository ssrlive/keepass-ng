use crate::db::{
    Attachment, AutoType, Color, CustomDataItem, History, IconId, Times, Value,
    node::{Node, NodePtr},
    rc_refcell_node,
};
use secrecy::ExposeSecret;
use std::collections::HashMap;
use uuid::Uuid;

/// A database entry containing several key-value fields.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Entry {
    pub(crate) uuid: Uuid,
    pub(crate) fields: HashMap<String, Value<String>>,
    pub(crate) autotype: Option<AutoType>,
    pub(crate) tags: Vec<String>,

    pub(crate) times: Times,

    pub(crate) custom_data: HashMap<String, CustomDataItem>,

    pub(crate) icon_id: Option<IconId>,
    pub(crate) custom_icon: Option<Uuid>,

    pub(crate) foreground_color: Option<Color>,
    pub(crate) background_color: Option<Color>,

    pub(crate) override_url: Option<String>,
    pub(crate) quality_check: Option<bool>,

    pub attachments: HashMap<String, Attachment>,

    pub(crate) history: Option<History>,

    pub(crate) parent: Option<Uuid>,

    pub(crate) previous_parent_group: Option<Uuid>,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            uuid: Uuid::new_v4(),
            fields: HashMap::new(),
            autotype: None,
            tags: Vec::new(),
            times: Times::new(),
            custom_data: Default::default(),
            icon_id: Some(IconId::KEY),
            custom_icon: None,
            foreground_color: None,
            background_color: None,
            override_url: None,
            quality_check: None,
            attachments: HashMap::new(),
            history: None,
            parent: None,
            previous_parent_group: None,
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
            && self.custom_icon == other.custom_icon
            && self.foreground_color == other.foreground_color
            && self.background_color == other.background_color
            && self.override_url == other.override_url
            && self.quality_check == other.quality_check
            && self.attachments == other.attachments
            && self.history == other.history
        // && self.parent == other.parent
    }
}

impl Eq for Entry {}

impl Node for Entry {
    fn duplicate(&self) -> NodePtr {
        let mut tmp = self.clone();
        tmp.parent = None;
        rc_refcell_node(tmp)
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
        self.custom_icon
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

impl Entry {
    pub fn quality_check(&self) -> bool {
        self.quality_check.unwrap_or(true)
    }

    pub fn previous_parent_group(&self) -> Option<Uuid> {
        self.previous_parent_group
    }

    pub fn custom_icon_uuid(&self) -> Option<Uuid> {
        self.custom_icon
    }

    pub fn custom_data(&self) -> &HashMap<String, CustomDataItem> {
        &self.custom_data
    }

    pub fn custom_data_mut(&mut self) -> &mut HashMap<String, CustomDataItem> {
        &mut self.custom_data
    }

    pub fn get_history(&self) -> &Option<History> {
        &self.history
    }

    pub fn purge_history(&mut self) {
        self.history = None;
    }

    pub(crate) fn set_unprotected_field_pair(&mut self, field_name: &str, field_value: Option<&str>) {
        if let Some(field_value) = field_value {
            let v = Value::Unprotected(field_value.to_string());
            self.fields.insert(field_name.to_string(), v);
        } else {
            self.fields.remove(field_name);
        }
    }

    pub(crate) fn set_protected_field_pair<T: AsRef<[u8]>>(&mut self, field_name: &str, field_value: Option<T>) {
        if let Some(field_value) = field_value {
            let value = String::from_utf8_lossy(field_value.as_ref()).into_owned();
            let v = Value::protected(value);
            self.fields.insert(field_name.to_string(), v);
        } else {
            self.fields.remove(field_name);
        }
    }

    pub(crate) fn set_binary_field_pair<T: AsRef<[u8]>>(&mut self, field_name: &str, field_value: Option<T>) {
        if let Some(field_value) = field_value {
            self.attachments.insert(
                field_name.to_string(),
                Attachment {
                    data: Value::unprotected(field_value.as_ref().to_vec()),
                },
            );
        } else {
            self.attachments.remove(field_name);
        }
    }
}

impl<'a> Entry {
    /// Get a field by name, taking care of unprotecting Protected values automatically
    pub fn get(&'a self, key: &str) -> Option<&'a str> {
        match self.fields.get(key) {
            None => None,
            Some(Value::Protected(pv)) => Some(pv.expose_secret()),
            Some(Value::Unprotected(uv)) => Some(uv),
        }
    }

    /// Get a bytes field by name
    pub fn get_bytes(&'a self, key: &str) -> Option<&'a [u8]> {
        self.attachments.get(key).map(|attachment| attachment.data.get().as_slice())
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

    #[rustfmt::skip]
    const EXCLUDED_FIELDS: [&'static str; 9] = ["Password", "BinaryData", "otp", "Title", "URL", "UserName", "Notes", "Additional", "BinaryDesc"];

    /// Set or remove additional attributes (custom string data)
    pub fn set_additional_attribute(&mut self, key: &str, value: Option<&str>) -> crate::Result<()> {
        if Self::EXCLUDED_FIELDS.contains(&key) {
            return Err(format!("Cannot set additional attribute for field {key}").into());
        }
        self.set_unprotected_field_pair(key, value);
        Ok(())
    }

    /// Get an additional attribute (custom string data)
    pub fn get_additional_attribute(&self, key: &str) -> Option<&str> {
        if Self::EXCLUDED_FIELDS.contains(&key) {
            return None;
        }
        self.get(key)
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
        self.set_protected_field_pair("Password", password.map(|p| p.as_bytes()));
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
    pub(crate) fn has_uncommited_changes(&self) -> bool {
        if let Some(history) = self.history.as_ref() {
            if history.entries.is_empty() {
                return true;
            }

            let new_times = Times::default();

            let mut sanitized_entry = self.clone();
            sanitized_entry.times = new_times.clone();
            sanitized_entry.history.take();

            let mut last_history_entry = history.entries.first().unwrap().clone();
            last_history_entry.times = new_times;
            last_history_entry.history.take();

            if sanitized_entry.eq(&last_history_entry) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod entry_tests {
    use super::{Entry, Node};
    use std::{thread, time};

    #[test]
    fn byte_values() {
        let mut entry = Entry::default();
        entry.set_binary_field_pair("a-bytes", Some(&[1, 2, 3]));

        entry.set_unprotected_field_pair("a-unprotected", Some("asdf"));
        entry.set_protected_field_pair("a-protected", Some("asdf".as_bytes()));

        assert_eq!(entry.get_bytes("a-bytes"), Some(&[1, 2, 3][..]));
        assert_eq!(entry.get_bytes("a-unprotected"), None);
        assert_eq!(entry.get_bytes("a-protected"), None);

        assert_eq!(entry.get("a-bytes"), None);

        assert!(!entry.attachments["a-bytes"].data.is_empty());
        entry.set_binary_field_pair::<&[u8]>("a-bytes", None);
        assert_eq!(entry.get_bytes("a-bytes"), None);
    }

    #[test]
    fn update_history() {
        let mut entry = Entry::default();
        let mut last_modification_time = entry.times.get_last_modification().unwrap();

        entry.set_username(Some("user"));
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
        entry.set_raw_otp_value(
            Some("otpauth://totp/ACME%20Co:john.doe@email.com?secret=HXDMVJECJJWSRB3HWIZR4IFUGFTMXBOZ&issuer=ACME%20Co&algorithm=SHA1&digits=6&period=30"),
        );

        assert!(entry.get_otp().is_ok());
    }

    #[cfg(feature = "serialization")]
    #[test]
    fn serialization() {
        use super::Value;
        assert_eq!(
            serde_json::to_string(&Value::Unprotected(vec![65, 66, 67])).unwrap(),
            "[65,66,67]".to_string()
        );

        assert_eq!(
            serde_json::to_string(&Value::Unprotected("ABC".to_string())).unwrap(),
            "\"ABC\"".to_string()
        );

        assert_eq!(
            serde_json::to_string(&Value::<String>::protected("ABC")).unwrap(),
            "\"ABC\"".to_string()
        );
    }
}
