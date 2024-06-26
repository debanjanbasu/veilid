use super::*;

/// Attachment abstraction for network 'signal strength'
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(
    target_arch = "wasm32",
    derive(Tsify),
    tsify(namespace, from_wasm_abi, into_wasm_abi)
)]
pub enum AttachmentState {
    Detached = 0,
    Attaching = 1,
    AttachedWeak = 2,
    AttachedGood = 3,
    AttachedStrong = 4,
    FullyAttached = 5,
    OverAttached = 6,
    Detaching = 7,
}

impl fmt::Display for AttachmentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let out = match self {
            AttachmentState::Attaching => "attaching".to_owned(),
            AttachmentState::AttachedWeak => "attached_weak".to_owned(),
            AttachmentState::AttachedGood => "attached_good".to_owned(),
            AttachmentState::AttachedStrong => "attached_strong".to_owned(),
            AttachmentState::FullyAttached => "fully_attached".to_owned(),
            AttachmentState::OverAttached => "over_attached".to_owned(),
            AttachmentState::Detaching => "detaching".to_owned(),
            AttachmentState::Detached => "detached".to_owned(),
        };
        write!(f, "{}", out)
    }
}

impl TryFrom<String> for AttachmentState {
    type Error = ();

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(match s.as_str() {
            "attaching" => AttachmentState::Attaching,
            "attached_weak" => AttachmentState::AttachedWeak,
            "attached_good" => AttachmentState::AttachedGood,
            "attached_strong" => AttachmentState::AttachedStrong,
            "fully_attached" => AttachmentState::FullyAttached,
            "over_attached" => AttachmentState::OverAttached,
            "detaching" => AttachmentState::Detaching,
            "detached" => AttachmentState::Detached,
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(target_arch = "wasm32", derive(Tsify))]
pub struct VeilidStateAttachment {
    pub state: AttachmentState,
    pub public_internet_ready: bool,
    pub local_network_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(target_arch = "wasm32", derive(Tsify))]
pub struct PeerTableData {
    #[schemars(with = "Vec<String>")]
    #[cfg_attr(target_arch = "wasm32", tsify(type = "string[]"))]
    pub node_ids: Vec<TypedKey>,
    pub peer_address: String,
    pub peer_stats: PeerStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(target_arch = "wasm32", derive(Tsify))]
pub struct VeilidStateNetwork {
    pub started: bool,
    pub bps_down: ByteCount,
    pub bps_up: ByteCount,
    pub peers: Vec<PeerTableData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(target_arch = "wasm32", derive(Tsify))]
pub struct VeilidRouteChange {
    #[schemars(with = "Vec<String>")]
    pub dead_routes: Vec<RouteId>,
    #[schemars(with = "Vec<String>")]
    pub dead_remote_routes: Vec<RouteId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(target_arch = "wasm32", derive(Tsify))]
pub struct VeilidStateConfig {
    pub config: VeilidConfigInner,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(target_arch = "wasm32", derive(Tsify))]
pub struct VeilidValueChange {
    #[schemars(with = "String")]
    pub key: TypedKey,
    pub subkeys: ValueSubkeyRangeSet,
    pub count: u32,
    pub value: Option<ValueData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(target_arch = "wasm32", derive(Tsify), tsify(into_wasm_abi))]
#[serde(tag = "kind")]
pub enum VeilidUpdate {
    Log(Box<VeilidLog>),
    AppMessage(Box<VeilidAppMessage>),
    AppCall(Box<VeilidAppCall>),
    Attachment(Box<VeilidStateAttachment>),
    Network(Box<VeilidStateNetwork>),
    Config(Box<VeilidStateConfig>),
    RouteChange(Box<VeilidRouteChange>),
    ValueChange(Box<VeilidValueChange>),
    Shutdown,
}
from_impl_to_jsvalue!(VeilidUpdate);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(target_arch = "wasm32", derive(Tsify), tsify(into_wasm_abi))]
pub struct VeilidState {
    pub attachment: Box<VeilidStateAttachment>,
    pub network: Box<VeilidStateNetwork>,
    pub config: Box<VeilidStateConfig>,
}
from_impl_to_jsvalue!(VeilidState);
