use super::*;

/// Represents the 5-tuple of an established connection
/// Not used to specify connections to create, that is reserved for DialInfo
///
/// ConnectionDescriptors should never be from unspecified local addresses for connection oriented protocols
/// If the medium does not allow local addresses, None should have been used or 'new_no_local'
/// If we are specifying only a port, then the socket's 'local_address()' should have been used, since an
/// established connection is always from a real address to another real address.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConnectionDescriptor {
    remote: PeerAddress,
    local: Option<SocketAddress>,
}

impl ConnectionDescriptor {
    pub fn new(remote: PeerAddress, local: SocketAddress) -> Self {
        assert!(!remote.protocol_type().is_ordered() || !local.address().is_unspecified());

        Self {
            remote,
            local: Some(local),
        }
    }
    pub fn new_no_local(remote: PeerAddress) -> Self {
        Self {
            remote,
            local: None,
        }
    }
    pub fn remote(&self) -> PeerAddress {
        self.remote
    }
    pub fn remote_address(&self) -> &SocketAddress {
        self.remote.socket_address()
    }
    pub fn local(&self) -> Option<SocketAddress> {
        self.local
    }
    pub fn protocol_type(&self) -> ProtocolType {
        self.remote.protocol_type()
    }
    pub fn address_type(&self) -> AddressType {
        self.remote.address_type()
    }
    pub fn make_dial_info_filter(&self) -> DialInfoFilter {
        DialInfoFilter::all()
            .with_protocol_type(self.protocol_type())
            .with_address_type(self.address_type())
    }
}

impl MatchesDialInfoFilter for ConnectionDescriptor {
    fn matches_filter(&self, filter: &DialInfoFilter) -> bool {
        if !filter.protocol_type_set.contains(self.protocol_type()) {
            return false;
        }
        if !filter.address_type_set.contains(self.address_type()) {
            return false;
        }
        true
    }
}
