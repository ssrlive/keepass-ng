use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
#[error("Cannot parse color: '{}'", .0)]
pub struct ParseColorError(pub String);

/// A color value for the Database, or Entry
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[cfg(feature = "serialization")]
impl serde::Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        if !str.starts_with('#') || str.len() != 7 {
            return Err(ParseColorError(str.to_string()));
        }

        let var = u64::from_str_radix(str.trim_start_matches('#'), 16).map_err(|_e| ParseColorError(str.to_string()))?;

        let r = ((var >> 16) & 0xff) as u8;
        let g = ((var >> 8) & 0xff) as u8;
        let b = (var & 0xff) as u8;

        Ok(Self { r, g, b })
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:0x}{:0x}{:0x}", self.r, self.g, self.b)
    }
}
