//! Custom serde (de)serializers for specific data formats in KeePass XML flavor.

/// base64-encoded binary data
pub mod cs_base64 {
    use base64::{Engine as _, engine::general_purpose as base64_engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64_engine::STANDARD.encode(data))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        base64_engine::STANDARD.decode(s).map_err(serde::de::Error::custom)
    }
}

/// "True"/"False" boolean strings
pub mod cs_bool {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &bool, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(if *data { "True" } else { "False" })
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err(serde::de::Error::custom(format!("invalid boolean string: {s}"))),
        }
    }
}

/// Optional "True"/"False" boolean strings
pub mod cs_opt_bool {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &Option<bool>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match data {
            Some(v) => serializer.serialize_str(if *v { "True" } else { "False" }),
            None => serializer.serialize_str(""),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        match value {
            None => Ok(None),
            Some(s) => {
                let s = s.trim();
                if s.is_empty() || s.eq_ignore_ascii_case("null") {
                    return Ok(None);
                }
                match s.to_lowercase().as_str() {
                    "true" | "1" => Ok(Some(true)),
                    "false" | "0" => Ok(Some(false)),
                    _ => Err(serde::de::Error::custom(format!("invalid boolean string: {s}"))),
                }
            }
        }
    }
}

/// Optional value that implements FromStr that may be missing empty, e.g. numbers
pub mod cs_opt_fromstr {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, T>(data: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        match data {
            Some(v) => serializer.serialize_some(v),
            None => serializer.serialize_str(""),
        }
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        match value {
            None => Ok(None),
            Some(s) => {
                let s = s.trim();
                if s.is_empty() {
                    return Ok(None);
                }
                s.parse::<T>()
                    .map(Some)
                    .map_err(|e| serde::de::Error::custom(format!("error parsing value: {e}")))
            }
        }
    }
}

/// Optional stringly value that may be missing or empty
/// (e.g. `<Name></Name>` or `<Name/>`)
pub mod cs_opt_string {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S, T>(data: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        match data {
            Some(v) => serializer.serialize_some(v),
            None => serializer.serialize_str(""),
        }
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let value = Option::<String>::deserialize(deserializer)?;
        match value {
            None => Ok(None),
            Some(s) => {
                if s.trim().is_empty() {
                    Ok(None)
                } else {
                    T::deserialize(serde::de::IntoDeserializer::<D::Error>::into_deserializer(s))
                        .map(Some)
                        .map_err(serde::de::Error::custom)
                }
            }
        }
    }
}
