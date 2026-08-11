use crate::{
    crypt::ciphers::Cipher,
    db::rc_refcell_node,
    format::xml_db::{
        UUID,
        custom_serde::{cs_opt_bool, cs_opt_fromstr, cs_opt_string},
        entry::EntryXml,
        meta::CustomDataXml,
        times::TimesXml,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "Group", rename_all = "PascalCase")]
pub(crate) struct GroupXml {
    #[serde(rename = "UUID")]
    pub uuid: UUID,

    #[serde(default, with = "cs_opt_string")]
    pub name: Option<String>,

    #[serde(default, with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(default, with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    #[serde(default, rename = "IconID", with = "cs_opt_fromstr", skip_serializing_if = "Option::is_none")]
    pub icon_id: Option<usize>,

    #[serde(
        default,
        rename = "CustomIconUUID",
        with = "cs_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_icon_uuid: Option<UUID>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub times: Option<TimesXml>,

    #[serde(default, rename = "IsExpanded", with = "cs_opt_bool", skip_serializing_if = "Option::is_none")]
    pub is_expanded: Option<bool>,

    #[serde(default, with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub default_auto_type_sequence: Option<String>,

    #[serde(default, with = "cs_opt_bool")]
    pub enable_auto_type: Option<bool>,

    #[serde(default, with = "cs_opt_bool")]
    pub enable_searching: Option<bool>,

    #[serde(default, with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub last_top_visible_entry: Option<UUID>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<CustomDataXml>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_parent_group: Option<UUID>,

    #[serde(default, rename = "$value")]
    pub children: Vec<GroupOrEntryXml>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum GroupOrEntryXml {
    Group(GroupXml),
    Entry(EntryXml),
}

impl GroupXml {
    pub(crate) fn xml_to_db_handle(
        self,
        target: &mut crate::db::Group,
        header_attachments: &[crate::db::Attachment],
        inner_decryptor: &mut dyn Cipher,
    ) -> std::io::Result<()> {
        target.name = self.name;
        target.notes = self.notes;
        target.tags = self
            .tags
            .map(|tags| {
                tags.split(';')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        target.icon_id = self.icon_id.and_then(|id| id.try_into().ok());

        target.custom_icon_uuid = self.custom_icon_uuid.map(|uuid| uuid.0);

        target.times = self.times.map(|t| t.into()).unwrap_or_default();
        target.is_expanded = self.is_expanded.unwrap_or_default();
        target.default_autotype_sequence = self.default_auto_type_sequence;
        target.enable_autotype = self.enable_auto_type;
        target.enable_searching = self.enable_searching;
        target.last_top_visible_entry = self.last_top_visible_entry.map(|u| u.0);
        target.custom_data = self.custom_data.map(Into::into).unwrap_or_default();
        target.previous_parent_group = self.previous_parent_group.map(|uuid| uuid.0);

        for child in self.children {
            match child {
                GroupOrEntryXml::Group(g) => {
                    let mut new_group = crate::db::Group {
                        uuid: g.uuid.0,
                        ..Default::default()
                    };

                    g.xml_to_db_handle(&mut new_group, header_attachments, inner_decryptor)?;
                    let new_group_ref = rc_refcell_node(new_group);
                    let index = target.get_children().len();
                    target.add_child(new_group_ref, index);
                }
                GroupOrEntryXml::Entry(e) => {
                    let mut new_entry = crate::db::Entry {
                        uuid: e.uuid.0,
                        ..Default::default()
                    };
                    e.xml_to_db_handle(&mut new_entry, header_attachments, inner_decryptor)?;
                    let new_entry_ref = rc_refcell_node(new_entry);
                    let index = target.get_children().len();
                    target.add_child(new_entry_ref, index);
                }
            }
        }

        Ok(())
    }

    #[cfg(feature = "save_kdbx4")]
    pub(crate) fn db_to_xml(
        source: &crate::db::Group,
        inner_cipher: &mut dyn Cipher,
        attachments: &mut Vec<crate::db::Attachment>,
    ) -> std::io::Result<Self> {
        let mut children = Vec::new();

        for child in source.get_children() {
            let child = child.borrow();
            if let Some(group) = child.downcast_ref::<crate::db::Group>() {
                let group = GroupXml::db_to_xml(group, inner_cipher, attachments).map_err(std::io::Error::other)?;
                children.push(GroupOrEntryXml::Group(group));
            } else if let Some(entry) = child.downcast_ref::<crate::db::Entry>() {
                let entry = EntryXml::db_to_xml(entry, inner_cipher, attachments).map_err(std::io::Error::other)?;
                children.push(GroupOrEntryXml::Entry(entry));
            } else {
                let msg = format!("Unknown child type in group: {:?}", child);
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, msg));
            }
        }

        let custom_data = if source.custom_data.is_empty() {
            None
        } else {
            Some(source.custom_data.clone().into())
        };

        Ok(GroupXml {
            uuid: UUID(source.uuid),
            name: source.name.clone(),
            notes: source.notes.clone(),
            tags: if source.tags.is_empty() {
                None
            } else {
                Some(source.tags.join(";"))
            },
            icon_id: source.icon_id.map(usize::from),
            custom_icon_uuid: source.custom_icon_uuid.map(UUID),
            times: Some(source.times.clone().into()),
            is_expanded: Some(source.is_expanded),
            default_auto_type_sequence: source.default_autotype_sequence.clone(),
            enable_auto_type: source.enable_autotype,
            enable_searching: source.enable_searching,
            last_top_visible_entry: source.last_top_visible_entry.map(UUID),
            custom_data,
            previous_parent_group: source.previous_parent_group.map(UUID),
            children,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Test<T>(T);

    #[test]
    fn test_deserialize_group() {
        let xml = r#"
        <Group>
            <UUID>AAECAwQFBgcICQoLDA0ODw==</UUID>
            <Name>Example Group</Name>
            <Notes>This is a test group.</Notes>
            <IconID>48</IconID>
            <CustomIconUUID>oaKjpLGywcLR0tPU1dbX2A==</CustomIconUUID>
            <Times>
                <CreationTime>2023-10-05T12:34:56Z</CreationTime>
                <LastModificationTime>2023-10-06T12:34:56Z</LastModificationTime>
                <LastAccessTime>2023-10-07T12:34:56Z</LastAccessTime>
                <ExpiryTime>2023-12-31T23:59:59Z</ExpiryTime>
                <Expires>True</Expires>
                <UsageCount>42</UsageCount>
                <LocationChanged>2023-10-08T12:34:56Z</LocationChanged>
            </Times>
            <IsExpanded>True</IsExpanded>
            <DefaultAutoTypeSequence>{USERNAME}{TAB}{PASSWORD}{ENTER}</DefaultAutoTypeSequence>
            <EnableAutoType>True</EnableAutoType>
            <EnableSearching>False</EnableSearching>
            <LastTopVisibleEntry>AAECAwQFBgcICQoLDA0ODw==</LastTopVisibleEntry>
            <CustomData>
                <Item>
                    <Key>example_key</Key>
                    <Value>example_value</Value>
                </Item>
            </CustomData>
            <Group>
                <UUID>AAECAwQFBgcICQoLDA0ODw==</UUID>
                <Name>Sub Group</Name>
                <IsExpanded>False</IsExpanded>
            </Group>
            <Entry>
                <UUID>AAECAwQFBgcICQoLDA0ODw==</UUID>
            </Entry>
            <Group>
                <UUID>AAECAwQFBgcICQoLDA0ODw==</UUID>
                <Name>Another Sub Group</Name>
            </Group>
            <Entry>
                <UUID>AAECAwQFBgcICQoLDA0ODw==</UUID>
            </Entry>
        </Group>"#;

        let group: Test<GroupXml> = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(group.0.uuid.0.to_string(), "00010203-0405-0607-0809-0a0b0c0d0e0f");
        assert_eq!(group.0.name.as_deref(), Some("Example Group"));
        assert_eq!(group.0.notes.unwrap(), "This is a test group.");
        assert_eq!(group.0.icon_id.unwrap(), 48);
        let uuid = group.0.custom_icon_uuid.unwrap().0.to_string();
        assert_eq!(uuid, "a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8");
        assert_eq!(group.0.is_expanded, Some(true));
        assert_eq!(group.0.default_auto_type_sequence.unwrap(), "{USERNAME}{TAB}{PASSWORD}{ENTER}");
        assert!(group.0.enable_auto_type.unwrap());
        assert!(!group.0.enable_searching.unwrap());
        assert!(group.0.custom_data.is_some());
        assert_eq!(group.0.children.len(), 4);
    }
}
