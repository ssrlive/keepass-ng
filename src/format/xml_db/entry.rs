#[cfg(feature = "save_kdbx4")]
use crate::crypt::CryptographyError;
#[cfg(feature = "save_kdbx4")]
use crate::format::xml_db::tags::join_tags;
use crate::{
    crypt::ciphers::Cipher,
    db::Color,
    format::xml_db::{
        UUID,
        custom_serde::{cs_bool, cs_opt_bool, cs_opt_fromstr, cs_opt_string},
        meta::CustomDataXml,
        tags::split_tags,
        times::TimesXml,
    },
};
use base64::{Engine as _, engine::general_purpose as base64_engine};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "Entry", rename_all = "PascalCase")]
pub(crate) struct EntryXml {
    #[serde(rename = "UUID")]
    pub uuid: UUID,

    #[serde(default, rename = "IconID", with = "cs_opt_fromstr", skip_serializing_if = "Option::is_none")]
    pub icon_id: Option<usize>,

    #[serde(
        default,
        rename = "CustomIconUUID",
        with = "cs_opt_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_icon_uuid: Option<UUID>,

    #[serde(default, with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<Color>,

    #[serde(default, with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub background_color: Option<Color>,

    #[serde(default, rename = "OverrideURL", with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub override_url: Option<String>,

    #[serde(default, with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    #[serde(default, with = "cs_opt_bool", skip_serializing_if = "Option::is_none")]
    pub quality_check: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_parent_group: Option<UUID>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub times: Option<TimesXml>,

    #[serde(default, rename = "String")]
    pub string_fields: Vec<StringFieldXml>,

    #[serde(default, rename = "Binary")]
    pub binary_fields: Vec<BinaryFieldXml>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_type: Option<AutoTypeXml>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<HistoryXml>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<CustomDataXml>,
}

impl EntryXml {
    pub(crate) fn xml_to_db_handle(
        self,
        target: &mut crate::db::Entry,
        header_attachments: &[crate::db::Attachment],
        inner_decryptor: &mut dyn Cipher,
    ) -> std::io::Result<()> {
        target.icon_id = self.icon_id.and_then(|id| id.try_into().ok());

        target.custom_icon = self.custom_icon_uuid.map(|uuid| uuid.0);

        target.foreground_color = self.foreground_color;
        target.background_color = self.background_color;
        target.override_url = self.override_url;
        target.quality_check = self.quality_check;
        target.previous_parent_group = self.previous_parent_group.map(|uuid| uuid.0);
        target.tags = self.tags.as_deref().map(split_tags).unwrap_or_default();

        target.times = self.times.map(|t| t.into()).unwrap_or_default();

        for field in self.string_fields {
            if let Some(fval) = &field.value.value {
                let value = if field.value.protected {
                    let fval = base64_engine::STANDARD.decode(fval).map_err(std::io::Error::other)?;
                    let fval = inner_decryptor.decrypt(&fval).map_err(std::io::Error::other)?;
                    let fval = String::from_utf8_lossy(&fval).to_string();

                    crate::db::Value::protected(fval)
                } else {
                    crate::db::Value::unprotected(fval)
                };
                target.fields.insert(field.key, value);
            }
        }

        for field in self.binary_fields {
            if let Some(attachment) = header_attachments.get(field.value.value_ref) {
                target.attachments.insert(field.key.clone(), attachment.clone());
            }
        }

        target.autotype = self.auto_type.map(|at| at.into());

        if let Some(h) = self.history {
            target.history = Some(crate::db::History {
                entries: h
                    .entries
                    .into_iter()
                    .map(|e| {
                        let mut he = crate::db::Entry {
                            uuid: e.uuid.0,
                            ..Default::default()
                        };

                        e.xml_to_db_handle(&mut he, header_attachments, inner_decryptor)?;
                        he.history = None; // history entries cannot have their own history
                        Ok(he)
                    })
                    .collect::<Result<_, std::io::Error>>()?,
            });
        }

        if let Some(cd) = self.custom_data {
            target.custom_data = cd.into();
        }

        Ok(())
    }

    #[cfg(feature = "save_kdbx4")]
    pub(crate) fn db_to_xml(
        db: &crate::db::Entry,
        inner_encryptor: &mut dyn Cipher,
        attachments: &mut Vec<crate::db::Attachment>,
    ) -> Result<Self, CryptographyError> {
        let custom_icon_uuid = db.custom_icon.map(UUID);

        let mut string_fields = Vec::with_capacity(db.fields.len());
        for (k, v) in &db.fields {
            let value = if v.is_protected() {
                let encrypted = inner_encryptor.encrypt(v.get().as_bytes())?;
                let encoded = base64_engine::STANDARD.encode(&encrypted);

                StringValueXml {
                    protected: true,
                    value: Some(encoded),
                }
            } else {
                StringValueXml {
                    protected: false,
                    value: Some(v.as_str().to_string()),
                }
            };

            string_fields.push(StringFieldXml { key: k.clone(), value });
        }

        let mut binary_fields = Vec::with_capacity(db.attachments.len());
        for (key, attachment) in &db.attachments {
            binary_fields.push(BinaryFieldXml {
                key: key.clone(),
                value: BinaryValueXml {
                    value_ref: attachments.len(),
                },
            });
            attachments.push(attachment.clone());
        }

        let history = if let Some(h) = db.history.as_ref() {
            Some(HistoryXml {
                entries: h
                    .entries
                    .iter()
                    .map(|e| EntryXml::db_to_xml(e, inner_encryptor, attachments))
                    .collect::<Result<_, CryptographyError>>()?,
            })
        } else {
            None
        };

        let custom_data: Option<CustomDataXml> = if db.custom_data.is_empty() {
            None
        } else {
            Some(db.custom_data.clone().into())
        };

        Ok(EntryXml {
            uuid: UUID(db.uuid),
            icon_id: db.icon_id.map(|id| id.into()),
            custom_icon_uuid,
            foreground_color: db.foreground_color,
            background_color: db.background_color,
            override_url: db.override_url.clone(),
            tags: join_tags(&db.tags),
            quality_check: db.quality_check,
            previous_parent_group: db.previous_parent_group.map(UUID),
            times: Some(db.times.clone().into()),
            string_fields,
            binary_fields,
            auto_type: db.autotype.as_ref().map(|at| at.clone().into()),
            history,
            custom_data,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct StringFieldXml {
    pub key: String,
    pub value: StringValueXml,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StringValueXml {
    #[serde(default, rename = "@Protected", with = "cs_bool")]
    protected: bool,

    #[serde(default, rename = "$value", with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    value: Option<String>,
}

impl Serialize for StringValueXml {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        if self.protected {
            let mut state = serializer.serialize_struct("StringValue", 2)?;
            state.serialize_field("@Protected", if self.protected { "True" } else { "False" })?;

            if let Some(ref val) = self.value {
                state.serialize_field("$value", val)?;
            } else {
                state.serialize_field("$value", "")?;
            }
            state.end()
        } else {
            let mut state = serializer.serialize_struct("StringValue", 1)?;

            if let Some(ref val) = self.value {
                state.serialize_field("$value", val)?;
            } else {
                state.serialize_field("$value", "")?;
            }
            state.end()
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct BinaryFieldXml {
    pub key: String,
    pub value: BinaryValueXml,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct BinaryValueXml {
    #[serde(rename = "@Ref")]
    pub value_ref: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct AutoTypeXml {
    #[serde(default, with = "cs_bool")]
    pub enabled: bool,

    #[serde(default, with = "cs_opt_fromstr", skip_serializing_if = "Option::is_none")]
    pub data_transfer_obfuscation: Option<usize>,

    #[serde(default, with = "cs_opt_string", skip_serializing_if = "Option::is_none")]
    pub default_sequence: Option<String>,

    #[serde(rename = "Association", default)]
    pub associations: Vec<AutoTypeAssociationXml>,
}

impl From<AutoTypeXml> for crate::db::AutoType {
    fn from(value: AutoTypeXml) -> Self {
        crate::db::AutoType {
            enabled: value.enabled,
            default_sequence: value.default_sequence,
            data_transfer_obfuscation: value.data_transfer_obfuscation.map(|d| d.into()).unwrap_or_default(),
            associations: value.associations.into_iter().map(|a| a.into()).collect(),
        }
    }
}

impl From<crate::db::AutoType> for AutoTypeXml {
    fn from(value: crate::db::AutoType) -> Self {
        Self {
            enabled: value.enabled,
            data_transfer_obfuscation: Some(value.data_transfer_obfuscation.into()),
            default_sequence: value.default_sequence,
            associations: value.associations.into_iter().map(|a| a.into()).collect(),
        }
    }
}

impl From<usize> for crate::db::DataTransferObfuscation {
    fn from(value: usize) -> Self {
        match value {
            0 => Self::None,
            1 => Self::UseClipboard,
            _ => Self::None, // default to None for unknown values
        }
    }
}

impl From<crate::db::DataTransferObfuscation> for usize {
    fn from(value: crate::db::DataTransferObfuscation) -> Self {
        match value {
            crate::db::DataTransferObfuscation::None => 0,
            crate::db::DataTransferObfuscation::UseClipboard => 1,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct AutoTypeAssociationXml {
    #[serde(default, with = "cs_opt_string")]
    pub window: Option<String>,

    #[serde(default, with = "cs_opt_string")]
    pub keystroke_sequence: Option<String>,
}

impl From<AutoTypeAssociationXml> for crate::db::AutoTypeAssociation {
    fn from(val: AutoTypeAssociationXml) -> Self {
        crate::db::AutoTypeAssociation {
            window: val.window,
            sequence: val.keystroke_sequence,
        }
    }
}

impl From<crate::db::AutoTypeAssociation> for AutoTypeAssociationXml {
    fn from(source: crate::db::AutoTypeAssociation) -> Self {
        Self {
            window: source.window,
            keystroke_sequence: source.sequence,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct HistoryXml {
    #[serde(default, rename = "Entry")]
    pub entries: Vec<EntryXml>,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[derive(Debug, Serialize, Deserialize)]
    struct Test<T>(T);

    #[test]
    fn test_deserialize_string_field() {
        let xml = r#"<String>
            <Key>Title</Key>
            <Value>Example Title</Value>
        </String>"#;

        let deserialized: Test<StringFieldXml> = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(deserialized.0.key, "Title");
        assert_eq!(deserialized.0.value.value.unwrap(), "Example Title");
        assert!(!deserialized.0.value.protected);

        let xml_protected = r#"<String>
            <Key>Password</Key>
            <Value Protected="True">cGFzc3dvcmQ=</Value>
        </String>"#;

        let deserialized_protected: Test<StringFieldXml> = quick_xml::de::from_str(xml_protected).unwrap();
        assert_eq!(deserialized_protected.0.key, "Password");
        assert_eq!(deserialized_protected.0.value.value.unwrap(), "cGFzc3dvcmQ=");
        assert!(deserialized_protected.0.value.protected);
    }

    #[test]
    fn test_serialize_string_field() {
        let string_field = StringFieldXml {
            key: "Username".to_string(),
            value: StringValueXml {
                protected: false,
                value: Some("user123".to_string()),
            },
        };

        let serialized = quick_xml::se::to_string(&Test(string_field)).unwrap();
        assert_eq!(serialized, r#"<Test><Key>Username</Key><Value>user123</Value></Test>"#);

        let string_field_protected = StringFieldXml {
            key: "Password".to_string(),
            value: StringValueXml {
                protected: true,
                value: Some("cGFzc3dvcmQ=".to_string()),
            },
        };

        let serialized_protected = quick_xml::se::to_string(&Test(string_field_protected)).unwrap();
        assert_eq!(
            serialized_protected,
            r#"<Test><Key>Password</Key><Value Protected="True">cGFzc3dvcmQ=</Value></Test>"#
        );
    }

    #[test]
    fn test_deserialize_binary_field() {
        let xml = r#"<Binary>
            <Key>Attachment</Key>
            <Value Ref="1"/>
        </Binary>"#;

        let deserialized: Test<BinaryFieldXml> = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(deserialized.0.key, "Attachment");
        assert_eq!(deserialized.0.value.value_ref, 1);
    }

    #[test]
    fn test_serialize_binary_field() {
        let binary_field = BinaryFieldXml {
            key: "Attachment".to_string(),
            value: BinaryValueXml { value_ref: 1 },
        };
        let serialized = quick_xml::se::to_string(&Test(binary_field)).unwrap();
        assert_eq!(serialized, r#"<Test><Key>Attachment</Key><Value Ref="1"/></Test>"#);
    }

    #[test]
    fn test_deserialize_autotype() {
        let xml = r#"
        <AutoType>
            <Enabled>True</Enabled>
            <DataTransferObfuscation>0</DataTransferObfuscation>
            <DefaultSequence>{USERNAME}{TAB}{PASSWORD}{ENTER}</DefaultSequence>
        </AutoType>"#;

        let deserialized: Test<AutoTypeXml> = quick_xml::de::from_str(xml).unwrap();
        assert!(deserialized.0.enabled);
        assert_eq!(deserialized.0.data_transfer_obfuscation, Some(0));
        assert_eq!(deserialized.0.default_sequence.unwrap(), "{USERNAME}{TAB}{PASSWORD}{ENTER}");
    }

    #[test]
    fn test_serialize_autotype() {
        let autotype = AutoTypeXml {
            enabled: true,
            data_transfer_obfuscation: Some(0),
            default_sequence: Some("{USERNAME}{TAB}{PASSWORD}{ENTER}".to_string()),
            associations: vec![AutoTypeAssociationXml {
                window: Some("Example Window".to_string()),
                keystroke_sequence: Some("{USERNAME}{TAB}{PASSWORD}{ENTER}".to_string()),
            }],
        };

        let serialized = quick_xml::se::to_string(&Test(autotype)).unwrap();
        assert_eq!(
            serialized,
            r#"<Test><Enabled>True</Enabled><DataTransferObfuscation>0</DataTransferObfuscation><DefaultSequence>{USERNAME}{TAB}{PASSWORD}{ENTER}</DefaultSequence><Association><Window>Example Window</Window><KeystrokeSequence>{USERNAME}{TAB}{PASSWORD}{ENTER}</KeystrokeSequence></Association></Test>"#
        );
    }

    #[test]
    fn test_deserialize_entry() {
        let xml = r#"
        <Entry>
            <UUID>AAECAwQFBgcICQoLDA0ODw==</UUID>
            <IconID>1</IconID>
            <ForegroundColor>#FF0000</ForegroundColor>
            <BackgroundColor>#00FF00</BackgroundColor>
            <OverrideURL>https://example.com</OverrideURL>
            <Tags>tag1;tag2</Tags>
            <Times>
                <CreationTime>2023-10-05T12:34:56Z</CreationTime>
                <LastModificationTime>2023-10-06T12:34:56Z</LastModificationTime>
                <LastAccessTime>2023-10-07T12:34:56Z</LastAccessTime>
                <ExpiryTime>2024-10-05T12:34:56Z</ExpiryTime>
                <Expires>True</Expires>
                <UsageCount>5</UsageCount>
                <LocationChanged>2023-10-08T12:34:56Z</LocationChanged>
            </Times>
            <String>
                <Key>Title</Key>
                <Value>Example Title</Value>
            </String>
            <Binary>
                <Key>Attachment</Key>
                <Value Ref="1"/>
            </Binary>
            <AutoType>
                <Enabled>True</Enabled>
                <DataTransferObfuscation>0</DataTransferObfuscation>
                <DefaultSequence>{USERNAME}{TAB}{PASSWORD}{ENTER}</DefaultSequence>
            </AutoType>
        </Entry>"#;

        let deserialized: Test<EntryXml> = quick_xml::de::from_str(xml).unwrap();
        assert_eq!(
            deserialized.0.uuid.0.as_bytes(),
            &[
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f
            ]
        );
        assert_eq!(deserialized.0.icon_id.unwrap(), 1);
        assert_eq!(deserialized.0.foreground_color.unwrap().to_string(), "#FF0000");
        assert_eq!(deserialized.0.background_color.unwrap().to_string(), "#00FF00");
        assert_eq!(deserialized.0.override_url.unwrap(), "https://example.com");
        assert_eq!(deserialized.0.tags.unwrap(), "tag1;tag2");
        assert_eq!(deserialized.0.string_fields.len(), 1);
        assert_eq!(deserialized.0.string_fields[0].key, "Title");
        assert_eq!(deserialized.0.string_fields[0].value.value.as_ref().unwrap(), "Example Title");
        assert_eq!(deserialized.0.binary_fields.len(), 1);
        assert_eq!(deserialized.0.binary_fields[0].key, "Attachment");
        assert_eq!(deserialized.0.binary_fields[0].value.value_ref, 1);
        assert!(deserialized.0.auto_type.is_some());
        let autotype = deserialized.0.auto_type.unwrap();
        assert!(autotype.enabled);
        assert_eq!(autotype.data_transfer_obfuscation, Some(0));
        assert_eq!(autotype.default_sequence.unwrap(), "{USERNAME}{TAB}{PASSWORD}{ENTER}");

        assert!(deserialized.0.history.is_none());
    }

    #[test]
    fn test_deserialize_entry_minimal() {
        let xml = r#"<Entry>
            <UUID>AAECAwQFBgcICQoLDA0ODw==</UUID>
            <IconID/>
            <ForegroundColor/>
            <BackgroundColor/>
            <OverrideURL/>
            <Tags/>
            <Times/>
            <AutoType/>
        </Entry>"#;

        let deserialized: Test<EntryXml> = quick_xml::de::from_str(xml).unwrap();

        println!("{:#?}", deserialized);

        assert!(deserialized.0.icon_id.is_none());
        assert!(deserialized.0.foreground_color.is_none());
        assert!(deserialized.0.background_color.is_none());
        assert!(deserialized.0.override_url.is_none());
        assert!(deserialized.0.tags.is_none());
        assert!(deserialized.0.string_fields.is_empty());
        assert!(deserialized.0.binary_fields.is_empty());
    }
}
