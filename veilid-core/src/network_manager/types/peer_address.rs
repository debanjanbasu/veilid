use super::*;

#[derive(
    Copy,
    Clone,
    Debug,
    PartialEq,
    PartialOrd,
    Eq,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    RkyvArchive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes))]
pub struct PeerAddress {
    protocol_type: ProtocolType,
    #[serde(with = "json_as_string")]
    socket_address: SocketAddress,
}

impl PeerAddress {
    pub fn new(socket_address: SocketAddress, protocol_type: ProtocolType) -> Self {
        Self {
            socket_address: socket_address.to_canonical(),
            protocol_type,
        }
    }

    pub fn socket_address(&self) -> &SocketAddress {
        &self.socket_address
    }

    pub fn protocol_type(&self) -> ProtocolType {
        self.protocol_type
    }

    pub fn to_socket_addr(&self) -> SocketAddr {
        self.socket_address.to_socket_addr()
    }

    pub fn address_type(&self) -> AddressType {
        self.socket_address.address_type()
    }
}

impl fmt::Display for PeerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.protocol_type, self.socket_address)
    }
}

impl FromStr for PeerAddress {
    type Err = VeilidAPIError;
    fn from_str(s: &str) -> VeilidAPIResult<PeerAddress> {
        let Some((first, second)) = s.split_once(':') else {
            return Err(VeilidAPIError::parse_error("PeerAddress is missing a colon: {}", s));
        };
        let protocol_type = ProtocolType::from_str(first)?;
        let socket_address = SocketAddress::from_str(second)?;
        Ok(PeerAddress::new(socket_address, protocol_type))
    }
}