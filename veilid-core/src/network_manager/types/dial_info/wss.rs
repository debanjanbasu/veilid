use super::*;

#[derive(
    Clone,
    Default,
    Debug,
    PartialEq,
    PartialOrd,
    Ord,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    RkyvArchive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes))]
pub struct DialInfoWSS {
    pub socket_address: SocketAddress,
    pub request: String,
}
