use super::*;

/// FOURCC code
#[derive(
    Copy,
    Default,
    Clone,
    Hash,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    RkyvArchive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes, PartialOrd, Ord, PartialEq, Eq, Hash))]
pub struct FourCC(pub [u8; 4]);

impl From<[u8; 4]> for FourCC {
    fn from(b: [u8; 4]) -> Self {
        Self(b)
    }
}
impl TryFrom<&[u8]> for FourCC {
    type Error = VeilidAPIError;
    fn try_from(b: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self(b.try_into().map_err(VeilidAPIError::generic)?))
    }
}

impl fmt::Display for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}
impl fmt::Debug for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

impl FromStr for FourCC {
    type Err = VeilidAPIError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            s.as_bytes().try_into().map_err(VeilidAPIError::generic)?,
        ))
    }
}
