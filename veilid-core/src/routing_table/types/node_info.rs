use super::*;

pub type Capability = FourCC;
pub const CAP_ROUTE: Capability = FourCC(*b"ROUT");
#[cfg(feature = "unstable-tunnels")]
pub const CAP_TUNNEL: Capability = FourCC(*b"TUNL");
pub const CAP_SIGNAL: Capability = FourCC(*b"SGNL");
pub const CAP_RELAY: Capability = FourCC(*b"RLAY");
pub const CAP_VALIDATE_DIAL_INFO: Capability = FourCC(*b"DIAL");
pub const CAP_DHT: Capability = FourCC(*b"DHTV");
pub const CAP_APPMESSAGE: Capability = FourCC(*b"APPM");
#[cfg(feature = "unstable-blockstore")]
pub const CAP_BLOCKSTORE: Capability = FourCC(*b"BLOC");

cfg_if! {
    if #[cfg(all(feature = "unstable-blockstore", feature="unstable-tunnels"))] {
        const PUBLIC_INTERNET_CAPABILITIES_LEN: usize = 8;
    } else if #[cfg(any(feature = "unstable-blockstore", feature="unstable-tunnels"))] {
        const PUBLIC_INTERNET_CAPABILITIES_LEN: usize = 7;
    } else  {
        const PUBLIC_INTERNET_CAPABILITIES_LEN: usize = 6;
    }
}
pub const PUBLIC_INTERNET_CAPABILITIES: [Capability; PUBLIC_INTERNET_CAPABILITIES_LEN] = [
    CAP_ROUTE,
    #[cfg(feature = "unstable-tunnels")]
    CAP_TUNNEL,
    CAP_SIGNAL,
    CAP_RELAY,
    CAP_VALIDATE_DIAL_INFO,
    CAP_DHT,
    CAP_APPMESSAGE,
    #[cfg(feature = "unstable-blockstore")]
    CAP_BLOCKSTORE,
];

#[cfg(feature = "unstable-blockstore")]
const LOCAL_NETWORK_CAPABILITIES_LEN: usize = 4;
#[cfg(not(feature = "unstable-blockstore"))]
const LOCAL_NETWORK_CAPABILITIES_LEN: usize = 3;

pub const LOCAL_NETWORK_CAPABILITIES: [Capability; LOCAL_NETWORK_CAPABILITIES_LEN] = [
    CAP_RELAY,
    CAP_DHT,
    CAP_APPMESSAGE,
    #[cfg(feature = "unstable-blockstore")]
    CAP_BLOCKSTORE,
];

pub const MAX_CAPABILITIES: usize = 64;

#[derive(
    Clone,
    Default,
    PartialEq,
    Eq,
    Debug,
    Serialize,
    Deserialize,
    RkyvArchive,
    RkyvSerialize,
    RkyvDeserialize,
)]
#[archive_attr(repr(C), derive(CheckBytes))]
pub struct NodeInfo {
    network_class: NetworkClass,
    #[with(RkyvEnumSet)]
    outbound_protocols: ProtocolTypeSet,
    #[with(RkyvEnumSet)]
    address_types: AddressTypeSet,
    envelope_support: Vec<u8>,
    crypto_support: Vec<CryptoKind>,
    capabilities: Vec<Capability>,
    dial_info_detail_list: Vec<DialInfoDetail>,
}

impl NodeInfo {
    pub fn new(
        network_class: NetworkClass,
        outbound_protocols: ProtocolTypeSet,
        address_types: AddressTypeSet,
        envelope_support: Vec<u8>,
        crypto_support: Vec<CryptoKind>,
        capabilities: Vec<Capability>,
        dial_info_detail_list: Vec<DialInfoDetail>,
    ) -> Self {
        Self {
            network_class,
            outbound_protocols,
            address_types,
            envelope_support,
            crypto_support,
            capabilities,
            dial_info_detail_list,
        }
    }

    pub fn network_class(&self) -> NetworkClass {
        self.network_class
    }
    pub fn outbound_protocols(&self) -> ProtocolTypeSet {
        self.outbound_protocols
    }
    pub fn address_types(&self) -> AddressTypeSet {
        self.address_types
    }
    pub fn envelope_support(&self) -> &[u8] {
        &self.envelope_support
    }
    pub fn crypto_support(&self) -> &[CryptoKind] {
        &self.crypto_support
    }
    pub fn capabilities(&self) -> &[Capability] {
        &self.capabilities
    }
    pub fn dial_info_detail_list(&self) -> &[DialInfoDetail] {
        &self.dial_info_detail_list
    }

    pub fn first_filtered_dial_info_detail<S, F>(
        &self,
        sort: Option<S>,
        filter: F,
    ) -> Option<DialInfoDetail>
    where
        S: Fn(&DialInfoDetail, &DialInfoDetail) -> std::cmp::Ordering,
        F: Fn(&DialInfoDetail) -> bool,
    {
        if let Some(sort) = sort {
            let mut dids = self.dial_info_detail_list.clone();
            dids.sort_by(sort);
            for did in dids {
                if filter(&did) {
                    return Some(did);
                }
            }
        } else {
            for did in &self.dial_info_detail_list {
                if filter(did) {
                    return Some(did.clone());
                }
            }
        };
        None
    }

    pub fn all_filtered_dial_info_details<S, F>(
        &self,
        sort: Option<S>,
        filter: F,
    ) -> Vec<DialInfoDetail>
    where
        S: Fn(&DialInfoDetail, &DialInfoDetail) -> std::cmp::Ordering,
        F: Fn(&DialInfoDetail) -> bool,
    {
        let mut dial_info_detail_list = Vec::new();

        if let Some(sort) = sort {
            let mut dids = self.dial_info_detail_list.clone();
            dids.sort_by(sort);
            for did in dids {
                if filter(&did) {
                    dial_info_detail_list.push(did);
                }
            }
        } else {
            for did in &self.dial_info_detail_list {
                if filter(did) {
                    dial_info_detail_list.push(did.clone());
                }
            }
        };
        dial_info_detail_list
    }

    /// Does this node has some dial info
    pub fn has_dial_info(&self) -> bool {
        !self.dial_info_detail_list.is_empty()
    }

    /// Is some relay required either for signal or inbound relay or outbound relay?
    pub fn requires_relay(&self) -> bool {
        match self.network_class {
            NetworkClass::InboundCapable => {
                for did in &self.dial_info_detail_list {
                    if did.class.requires_relay() {
                        return true;
                    }
                }
            }
            NetworkClass::OutboundOnly => {
                return true;
            }
            NetworkClass::WebApp => {
                return true;
            }
            NetworkClass::Invalid => {}
        }
        false
    }

    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }
    pub fn has_capabilities(&self, capabilities: &[Capability]) -> bool {
        for cap in capabilities {
            if !self.has_capability(*cap) {
                return false;
            }
        }
        true
    }

    /// Can this node assist with signalling? Yes but only if it doesn't require signalling, itself.
    /// Also used to determine if nodes are capable of validation of dial info, as that operation
    /// has the same requirements, inbound capability and a dial info that requires no assistance
    pub fn is_signal_capable(&self) -> bool {
        // Has capability?
        if !self.has_capability(CAP_SIGNAL) {
            return false;
        }

        // Must be inbound capable
        if !matches!(self.network_class, NetworkClass::InboundCapable) {
            return false;
        }
        // Do any of our dial info require signalling? if so, we can't offer signalling
        for did in &self.dial_info_detail_list {
            if did.class.requires_signal() {
                return false;
            }
        }
        true
    }
}
