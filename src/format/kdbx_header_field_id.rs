use crate::error::DatabaseIntegrityError;
use std::convert::TryFrom;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KDBXHeaderFieldID {
    EndOfHeader = 0,
    Comment = 1,
    CipherID = 2,
    CompressionFlags = 3,
    MasterSeed = 4,
    TransformSeed = 5,   // KDBX 3.1, for backward compatibility only
    TransformRounds = 6, // KDBX 3.1, for backward compatibility only
    EncryptionIV = 7,
    InnerRandomStreamKey = 8, // KDBX 3.1, for backward compatibility only
    StreamStartBytes = 9,     // KDBX 3.1, for backward compatibility only
    InnerRandomStreamID = 10, // KDBX 3.1, for backward compatibility only
    KdfParameters = 11,       // KDBX 4, superseding Transform*
    PublicCustomData = 12,    // KDBX 4
}

impl TryFrom<u8> for KDBXHeaderFieldID {
    type Error = DatabaseIntegrityError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(KDBXHeaderFieldID::EndOfHeader),
            1 => Ok(KDBXHeaderFieldID::Comment),
            2 => Ok(KDBXHeaderFieldID::CipherID),
            3 => Ok(KDBXHeaderFieldID::CompressionFlags),
            4 => Ok(KDBXHeaderFieldID::MasterSeed),
            5 => Ok(KDBXHeaderFieldID::TransformSeed),
            6 => Ok(KDBXHeaderFieldID::TransformRounds),
            7 => Ok(KDBXHeaderFieldID::EncryptionIV),
            8 => Ok(KDBXHeaderFieldID::InnerRandomStreamKey),
            9 => Ok(KDBXHeaderFieldID::StreamStartBytes),
            10 => Ok(KDBXHeaderFieldID::InnerRandomStreamID),
            11 => Ok(KDBXHeaderFieldID::KdfParameters),
            12 => Ok(KDBXHeaderFieldID::PublicCustomData),
            _ => Err(DatabaseIntegrityError::InvalidKDBXHeaderFieldID { field_id: value }),
        }
    }
}

impl TryFrom<&u8> for KDBXHeaderFieldID {
    type Error = DatabaseIntegrityError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        KDBXHeaderFieldID::try_from(*value)
    }
}

impl From<KDBXHeaderFieldID> for u8 {
    fn from(value: KDBXHeaderFieldID) -> Self {
        match value {
            KDBXHeaderFieldID::EndOfHeader => 0,
            KDBXHeaderFieldID::Comment => 1,
            KDBXHeaderFieldID::CipherID => 2,
            KDBXHeaderFieldID::CompressionFlags => 3,
            KDBXHeaderFieldID::MasterSeed => 4,
            KDBXHeaderFieldID::TransformSeed => 5,
            KDBXHeaderFieldID::TransformRounds => 6,
            KDBXHeaderFieldID::EncryptionIV => 7,
            KDBXHeaderFieldID::InnerRandomStreamKey => 8,
            KDBXHeaderFieldID::StreamStartBytes => 9,
            KDBXHeaderFieldID::InnerRandomStreamID => 10,
            KDBXHeaderFieldID::KdfParameters => 11,
            KDBXHeaderFieldID::PublicCustomData => 12,
        }
    }
}
