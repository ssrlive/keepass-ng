use chrono::NaiveDateTime;
use std::collections::HashMap;

/// Timestamps for a Group or Entry
#[derive(Debug, Default, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct Times {
    /// Does this node expire
    pub(crate) expires: bool,

    /// Number of usages
    pub(crate) usage_count: usize,

    /// Using chrono::NaiveDateTime which does not include timezone
    /// or UTC offset because KeePass clients typically store timestamps
    /// relative to the local time on the machine writing the data without
    /// including accurate UTC offset or timezone information.
    pub(crate) times: HashMap<String, NaiveDateTime>,
}

pub const EXPIRY_TIME_TAG_NAME: &str = "ExpiryTime";
pub const LAST_MODIFICATION_TIME_TAG_NAME: &str = "LastModificationTime";
pub const CREATION_TIME_TAG_NAME: &str = "CreationTime";
pub const LAST_ACCESS_TIME_TAG_NAME: &str = "LastAccessTime";
pub const LOCATION_CHANGED_TAG_NAME: &str = "LocationChanged";

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn now_timestamp() -> i64 {
    // Use JS Date.now() to get the current time in milliseconds, then convert it to seconds.
    let millis = js_sys::Date::now();
    (millis / 1000.0) as i64
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn now_timestamp() -> i64 {
    chrono::Utc::now().timestamp()
}

impl Times {
    fn get(&self, key: &str) -> Option<NaiveDateTime> {
        self.times.get(key).copied()
    }

    fn set(&mut self, key: &str, time: Option<NaiveDateTime>) {
        if let Some(time) = time {
            self.times.insert(key.to_string(), time);
        } else {
            self.times.remove(key);
        }
    }

    pub fn get_expires(&self) -> bool {
        self.expires
    }

    pub fn set_expires(&mut self, expires: bool) {
        self.expires = expires;
    }

    pub fn get_usage_count(&self) -> usize {
        self.usage_count
    }

    pub fn set_usage_count(&mut self, usage_count: usize) {
        self.usage_count = usage_count;
    }

    /// Convenience method for getting the time that the entry expires.
    /// This value is usually only meaningful/useful when expires == true
    pub fn get_expiry_time(&self) -> Option<NaiveDateTime> {
        self.get(EXPIRY_TIME_TAG_NAME)
    }

    pub fn set_expiry_time(&mut self, time: Option<NaiveDateTime>) {
        self.set(EXPIRY_TIME_TAG_NAME, time);
    }

    pub fn get_last_modification(&self) -> Option<NaiveDateTime> {
        self.get(LAST_MODIFICATION_TIME_TAG_NAME)
    }

    pub fn set_last_modification(&mut self, time: Option<NaiveDateTime>) {
        self.set(LAST_MODIFICATION_TIME_TAG_NAME, time);
    }

    pub fn get_creation(&self) -> Option<NaiveDateTime> {
        self.get(CREATION_TIME_TAG_NAME)
    }

    pub fn set_creation(&mut self, time: Option<NaiveDateTime>) {
        self.set(CREATION_TIME_TAG_NAME, time);
    }

    pub fn get_last_access(&self) -> Option<NaiveDateTime> {
        self.get(LAST_ACCESS_TIME_TAG_NAME)
    }

    pub fn set_last_access(&mut self, time: Option<NaiveDateTime>) {
        self.set(LAST_ACCESS_TIME_TAG_NAME, time);
    }

    pub fn get_location_changed(&self) -> Option<NaiveDateTime> {
        self.get(LOCATION_CHANGED_TAG_NAME)
    }

    pub fn set_location_changed(&mut self, time: Option<NaiveDateTime>) {
        self.set(LOCATION_CHANGED_TAG_NAME, time);
    }

    // Returns the current time, without the nanoseconds since
    // the last leap second.
    pub fn now() -> NaiveDateTime {
        let now = now_timestamp();
        chrono::DateTime::from_timestamp(now, 0).unwrap().naive_utc()
    }

    pub fn epoch() -> NaiveDateTime {
        chrono::DateTime::from_timestamp(0, 0).unwrap().naive_utc()
    }

    pub fn new() -> Times {
        let mut response = Times::default();
        let now = Some(Times::now());
        response.set_creation(now);
        response.set_last_modification(now);
        response.set_last_access(now);
        response.set_location_changed(now);
        response.set_expiry_time(now);
        response.set_expires(false);
        response
    }
}
