use chrono::NaiveDateTime;
use std::ops::{Deref, DerefMut};
use uuid::Uuid;

use crate::db::IconId;

/// Icon specification for an [Entry][crate::db::Entry] or [Group][crate::db::Group].
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub enum Icon {
    /// The icon is a built-in icon specified by an index
    BuiltIn(IconId),

    /// The icon is a custom icon specified by a UUID
    Custom(Uuid),
}

impl From<IconId> for Icon {
    fn from(icon_id: IconId) -> Self {
        Icon::BuiltIn(icon_id)
    }
}

impl From<Uuid> for Icon {
    fn from(uuid: Uuid) -> Self {
        Icon::Custom(uuid)
    }
}

impl TryFrom<Icon> for IconId {
    type Error = std::io::Error;
    fn try_from(icon: Icon) -> Result<Self, Self::Error> {
        match icon {
            Icon::BuiltIn(icon_id) => Ok(icon_id),
            Icon::Custom(_) => Err(std::io::Error::other("Custom icon cannot be converted to IconId")),
        }
    }
}

impl TryFrom<Icon> for Uuid {
    type Error = std::io::Error;
    fn try_from(icon: Icon) -> Result<Self, Self::Error> {
        match icon {
            Icon::BuiltIn(_) => Err(std::io::Error::other("Built-in icon cannot be converted to Uuid")),
            Icon::Custom(uuid) => Ok(uuid),
        }
    }
}

/// A custom icon stored in the database, containing raw image data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serialization", derive(serde::Serialize))]
pub struct CustomIcon {
    pub(crate) id: Uuid,

    /// Filename for the icon
    pub name: Option<String>,

    /// Last modification timestamp
    pub last_modification_time: Option<NaiveDateTime>,

    /// The raw image data
    pub data: Vec<u8>,
}

impl CustomIcon {
    /// Get the ID of this custom icon
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn new(id: Uuid, name: Option<String>, last_modification_time: Option<NaiveDateTime>, data: Vec<u8>) -> Self {
        Self {
            id,
            name,
            last_modification_time,
            data,
        }
    }
}

impl Deref for CustomIcon {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for CustomIcon {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
