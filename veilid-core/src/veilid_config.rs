use crate::dht::key;
use crate::intf;
use crate::xx::*;

use serde::*;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        pub type ConfigCallbackReturn = Result<Box<dyn core::any::Any>, String>;
        pub type ConfigCallback = Arc<dyn Fn(String) -> ConfigCallbackReturn>;

    } else {
        pub type ConfigCallbackReturn = Result<Box<dyn core::any::Any + Send>, String>;
        pub type ConfigCallback = Arc<dyn Fn(String) -> ConfigCallbackReturn + Send + Sync>;
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigHTTPS {
    pub enabled: bool,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>, // Fixed URL is not optional for TLS-based protocols and is dynamically validated
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigHTTP {
    pub enabled: bool,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigApplication {
    pub https: VeilidConfigHTTPS,
    pub http: VeilidConfigHTTP,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigUDP {
    pub enabled: bool,
    pub socket_pool_size: u32,
    pub listen_address: String,
    pub public_address: Option<String>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigTCP {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: String,
    pub public_address: Option<String>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigWS {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigWSS {
    pub connect: bool,
    pub listen: bool,
    pub max_connections: u32,
    pub listen_address: String,
    pub path: String,
    pub url: Option<String>, // Fixed URL is not optional for TLS-based protocols and is dynamically validated
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigProtocol {
    pub udp: VeilidConfigUDP,
    pub tcp: VeilidConfigTCP,
    pub ws: VeilidConfigWS,
    pub wss: VeilidConfigWSS,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigTLS {
    pub certificate_path: String,
    pub private_key_path: String,
    pub connection_initial_timeout_ms: u32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigDHT {
    pub resolve_node_timeout_ms: Option<u32>,
    pub resolve_node_count: u32,
    pub resolve_node_fanout: u32,
    pub max_find_node_count: u32,
    pub get_value_timeout_ms: Option<u32>,
    pub get_value_count: u32,
    pub get_value_fanout: u32,
    pub set_value_timeout_ms: Option<u32>,
    pub set_value_count: u32,
    pub set_value_fanout: u32,
    pub min_peer_count: u32,
    pub min_peer_refresh_time_ms: u32,
    pub validate_dial_info_receipt_time_ms: u32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigRPC {
    pub concurrency: u32,
    pub queue_size: u32,
    pub max_timestamp_behind_ms: Option<u32>,
    pub max_timestamp_ahead_ms: Option<u32>,
    pub timeout_ms: u32,
    pub max_route_hop_count: u8,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigLeases {
    pub max_server_signal_leases: u32,
    pub max_server_relay_leases: u32,
    pub max_client_signal_leases: u32,
    pub max_client_relay_leases: u32,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigNetwork {
    pub max_connections: u32,
    pub connection_initial_timeout_ms: u32,
    pub node_id: key::DHTKey,
    pub node_id_secret: key::DHTKeySecret,
    pub bootstrap: Vec<String>,
    pub rpc: VeilidConfigRPC,
    pub dht: VeilidConfigDHT,
    pub upnp: bool,
    pub natpmp: bool,
    pub enable_local_peer_scope: bool,
    pub restricted_nat_retries: u32,
    pub tls: VeilidConfigTLS,
    pub application: VeilidConfigApplication,
    pub protocol: VeilidConfigProtocol,
    pub leases: VeilidConfigLeases,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigTableStore {
    pub directory: String,
    pub delete: bool,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigBlockStore {
    pub directory: String,
    pub delete: bool,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigProtectedStore {
    pub allow_insecure_fallback: bool,
    pub always_use_insecure_storage: bool,
    pub insecure_fallback_directory: String,
    pub delete: bool,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigCapabilities {
    pub protocol_udp: bool,
    pub protocol_connect_tcp: bool,
    pub protocol_accept_tcp: bool,
    pub protocol_connect_ws: bool,
    pub protocol_accept_ws: bool,
    pub protocol_connect_wss: bool,
    pub protocol_accept_wss: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum VeilidConfigLogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl VeilidConfigLogLevel {
    pub fn to_level_filter(&self) -> LevelFilter {
        match self {
            Self::Off => LevelFilter::Off,
            Self::Error => LevelFilter::Error,
            Self::Warn => LevelFilter::Warn,
            Self::Info => LevelFilter::Info,
            Self::Debug => LevelFilter::Debug,
            Self::Trace => LevelFilter::Trace,
        }
    }
}
impl Default for VeilidConfigLogLevel {
    fn default() -> Self {
        Self::Off
    }
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct VeilidConfigInner {
    pub program_name: String,
    pub namespace: String,
    pub api_log_level: VeilidConfigLogLevel,
    pub capabilities: VeilidConfigCapabilities,
    pub protected_store: VeilidConfigProtectedStore,
    pub table_store: VeilidConfigTableStore,
    pub block_store: VeilidConfigBlockStore,
    pub network: VeilidConfigNetwork,
}

#[derive(Clone)]
pub struct VeilidConfig {
    inner: Arc<RwLock<VeilidConfigInner>>,
}

impl Default for VeilidConfig {
    fn default() -> Self {
        Self::new()
    }
}
impl VeilidConfig {
    fn new_inner() -> VeilidConfigInner {
        VeilidConfigInner::default()
    }

    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Self::new_inner())),
        }
    }

    pub fn setup_from_json(&mut self, config: String) -> Result<(), String> {
        {
            let mut inner = self.inner.write();
            *inner = serde_json::from_str(&config).map_err(map_to_string)?;
        }

        // Validate settings
        self.validate()?;

        Ok(())
    }

    pub fn setup(&mut self, cb: ConfigCallback) -> Result<(), String> {
        macro_rules! get_config {
            ($key:expr) => {
                let keyname = &stringify!($key)[6..];
                $key = *cb(keyname.to_owned())?.downcast().map_err(|_| {
                    let err = format!("incorrect type for key: {}", keyname);
                    debug!("{}", err);
                    err
                })?;
            };
        }
        {
            let mut inner = self.inner.write();
            get_config!(inner.program_name);
            get_config!(inner.namespace);
            get_config!(inner.api_log_level);
            get_config!(inner.capabilities.protocol_udp);
            get_config!(inner.capabilities.protocol_connect_tcp);
            get_config!(inner.capabilities.protocol_accept_tcp);
            get_config!(inner.capabilities.protocol_connect_ws);
            get_config!(inner.capabilities.protocol_accept_ws);
            get_config!(inner.capabilities.protocol_connect_wss);
            get_config!(inner.capabilities.protocol_accept_wss);
            get_config!(inner.table_store.directory);
            get_config!(inner.table_store.delete);
            get_config!(inner.block_store.directory);
            get_config!(inner.block_store.delete);
            get_config!(inner.protected_store.allow_insecure_fallback);
            get_config!(inner.protected_store.always_use_insecure_storage);
            get_config!(inner.protected_store.insecure_fallback_directory);
            get_config!(inner.protected_store.delete);
            get_config!(inner.network.node_id);
            get_config!(inner.network.node_id_secret);
            get_config!(inner.network.max_connections);
            get_config!(inner.network.connection_initial_timeout_ms);
            get_config!(inner.network.bootstrap);
            get_config!(inner.network.dht.resolve_node_timeout_ms);
            get_config!(inner.network.dht.resolve_node_count);
            get_config!(inner.network.dht.resolve_node_fanout);
            get_config!(inner.network.dht.max_find_node_count);
            get_config!(inner.network.dht.get_value_timeout_ms);
            get_config!(inner.network.dht.get_value_count);
            get_config!(inner.network.dht.get_value_fanout);
            get_config!(inner.network.dht.set_value_timeout_ms);
            get_config!(inner.network.dht.set_value_count);
            get_config!(inner.network.dht.set_value_fanout);
            get_config!(inner.network.dht.min_peer_count);
            get_config!(inner.network.dht.min_peer_refresh_time_ms);
            get_config!(inner.network.dht.validate_dial_info_receipt_time_ms);
            get_config!(inner.network.rpc.concurrency);
            get_config!(inner.network.rpc.queue_size);
            get_config!(inner.network.rpc.max_timestamp_behind_ms);
            get_config!(inner.network.rpc.max_timestamp_ahead_ms);
            get_config!(inner.network.rpc.timeout_ms);
            get_config!(inner.network.rpc.max_route_hop_count);
            get_config!(inner.network.upnp);
            get_config!(inner.network.natpmp);
            get_config!(inner.network.enable_local_peer_scope);
            get_config!(inner.network.restricted_nat_retries);
            get_config!(inner.network.tls.certificate_path);
            get_config!(inner.network.tls.private_key_path);
            get_config!(inner.network.tls.connection_initial_timeout_ms);
            get_config!(inner.network.application.https.enabled);
            get_config!(inner.network.application.https.listen_address);
            get_config!(inner.network.application.https.path);
            get_config!(inner.network.application.https.url);
            get_config!(inner.network.application.http.enabled);
            get_config!(inner.network.application.http.listen_address);
            get_config!(inner.network.application.http.path);
            get_config!(inner.network.application.http.url);
            get_config!(inner.network.protocol.udp.enabled);
            get_config!(inner.network.protocol.udp.socket_pool_size);
            get_config!(inner.network.protocol.udp.listen_address);
            get_config!(inner.network.protocol.udp.public_address);
            get_config!(inner.network.protocol.tcp.connect);
            get_config!(inner.network.protocol.tcp.listen);
            get_config!(inner.network.protocol.tcp.max_connections);
            get_config!(inner.network.protocol.tcp.listen_address);
            get_config!(inner.network.protocol.tcp.public_address);
            get_config!(inner.network.protocol.ws.connect);
            get_config!(inner.network.protocol.ws.listen);
            get_config!(inner.network.protocol.ws.max_connections);
            get_config!(inner.network.protocol.ws.listen_address);
            get_config!(inner.network.protocol.ws.path);
            get_config!(inner.network.protocol.ws.url);
            get_config!(inner.network.protocol.wss.connect);
            get_config!(inner.network.protocol.wss.listen);
            get_config!(inner.network.protocol.wss.max_connections);
            get_config!(inner.network.protocol.wss.listen_address);
            get_config!(inner.network.protocol.wss.path);
            get_config!(inner.network.protocol.wss.url);
            get_config!(inner.network.leases.max_server_signal_leases);
            get_config!(inner.network.leases.max_server_relay_leases);
            get_config!(inner.network.leases.max_client_signal_leases);
            get_config!(inner.network.leases.max_client_relay_leases);
        }
        // Validate settings
        self.validate()?;

        Ok(())
    }

    pub fn get(&self) -> RwLockReadGuard<VeilidConfigInner> {
        self.inner.read()
    }

    pub fn get_mut(&self) -> RwLockWriteGuard<VeilidConfigInner> {
        self.inner.write()
    }

    pub fn get_key_json(&self, key: &str) -> Result<String, String> {
        let c = self.get();

        // Generate json from whole config
        let jc = serde_json::to_string(&*c).map_err(map_to_string)?;
        let jvc = json::parse(&jc).map_err(map_to_string)?;

        // Find requested subkey
        if key.is_empty() {
            Ok(jvc.to_string())
        } else {
            // Split key into path parts
            let keypath: Vec<&str> = key.split('.').collect();
            let mut out = &jvc;
            for k in keypath {
                if !out.has_key(k) {
                    return Err(format!("invalid subkey '{}' in key '{}'", k, key));
                }
                out = &out[k];
            }
            Ok(out.to_string())
        }
    }
    pub fn set_key_json(&self, key: &str, value: &str) -> Result<(), String> {
        let mut c = self.get_mut();

        // Split key into path parts
        let keypath: Vec<&str> = key.split('.').collect();

        // Convert value into jsonvalue
        let newval = json::parse(value).map_err(map_to_string)?;

        // Generate json from whole config
        let jc = serde_json::to_string(&*c).map_err(map_to_string)?;
        let mut jvc = json::parse(&jc).map_err(map_to_string)?;

        // Find requested subkey
        let newconfigstring = if let Some((objkeyname, objkeypath)) = keypath.split_last() {
            // Replace subkey
            let mut out = &mut jvc;
            for k in objkeypath {
                if !out.has_key(*k) {
                    return Err(format!("invalid subkey '{}' in key '{}'", *k, key));
                }
                out = &mut out[*k];
            }
            if !out.has_key(objkeyname) {
                return Err(format!("invalid subkey '{}' in key '{}'", objkeyname, key));
            }
            out[*objkeyname] = newval;
            jvc.to_string()
        } else {
            newval.to_string()
        };
        // Generate and validate new config
        let mut newconfig = VeilidConfig::new();
        newconfig.setup_from_json(newconfigstring)?;
        //  Replace whole config
        *c = newconfig.get().clone();
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        let inner = self.inner.read();

        if inner.program_name.is_empty() {
            return Err("Program name must not be empty in 'program_name'".to_owned());
        }

        // if inner.network.protocol.udp.enabled {
        //     // Validate UDP settings
        // }
        if inner.network.protocol.tcp.listen {
            // Validate TCP settings
            if inner.network.protocol.tcp.max_connections == 0 {
                return Err("TCP max connections must be > 0 in config key 'network.protocol.tcp.max_connections'".to_owned());
            }
        }
        if inner.network.protocol.ws.listen {
            // Validate WS settings
            if inner.network.protocol.ws.max_connections == 0 {
                return Err("WS max connections must be > 0 in config key 'network.protocol.ws.max_connections'".to_owned());
            }
            if inner.network.application.https.enabled
                && inner.network.application.https.path == inner.network.protocol.ws.path
            {
                return Err("WS path conflicts with HTTPS application path in config key 'network.protocol.ws.path'".to_owned());
            }
            if inner.network.application.http.enabled
                && inner.network.application.http.path == inner.network.protocol.ws.path
            {
                return Err("WS path conflicts with HTTP application path in config key 'network.protocol.ws.path'".to_owned());
            }
        }
        if inner.network.protocol.wss.listen {
            // Validate WSS settings
            if inner.network.protocol.wss.max_connections == 0 {
                return Err("WSS max connections must be > 0 in config key 'network.protocol.wss.max_connections'".to_owned());
            }
            if inner
                .network
                .protocol
                .wss
                .url
                .as_ref()
                .map(|u| u.is_empty())
                .unwrap_or_default()
            {
                return Err(
                    "WSS URL must be specified in config key 'network.protocol.wss.url'".to_owned(),
                );
            }
            if inner.network.application.https.enabled
                && inner.network.application.https.path == inner.network.protocol.wss.path
            {
                return Err("WSS path conflicts with HTTPS application path in config key 'network.protocol.ws.path'".to_owned());
            }
            if inner.network.application.http.enabled
                && inner.network.application.http.path == inner.network.protocol.wss.path
            {
                return Err("WSS path conflicts with HTTP application path in config key 'network.protocol.ws.path'".to_owned());
            }
        }
        if inner.network.application.https.enabled {
            // Validate HTTPS settings
            if inner
                .network
                .application
                .https
                .url
                .as_ref()
                .map(|u| u.is_empty())
                .unwrap_or_default()
            {
                return Err(
                    "HTTPS URL must be specified in config key 'network.application.https.url'"
                        .to_owned(),
                );
            }
        }
        Ok(())
    }

    // Get the node id from config if one is specified
    // Must be done -after- protected store startup
    pub async fn init_node_id(&self, protected_store: intf::ProtectedStore) -> Result<(), String> {
        let mut node_id = self.inner.read().network.node_id;
        let mut node_id_secret = self.inner.read().network.node_id_secret;
        // See if node id was previously stored in the protected store
        if !node_id.valid {
            debug!("pulling node id from storage");
            if let Some(s) = protected_store.load_user_secret_string("node_id").await? {
                debug!("node id found in storage");
                node_id = key::DHTKey::try_decode(s.as_str())?
            } else {
                debug!("node id not found in storage");
            }
        }

        // See if node id secret was previously stored in the protected store
        if !node_id_secret.valid {
            debug!("pulling node id secret from storage");
            if let Some(s) = protected_store
                .load_user_secret_string("node_id_secret")
                .await?
            {
                debug!("node id secret found in storage");
                node_id_secret = key::DHTKeySecret::try_decode(s.as_str())?
            } else {
                debug!("node id secret not found in storage");
            }
        }

        // If we have a node id from storage, check it
        if node_id.valid && node_id_secret.valid {
            // Validate node id
            if !key::validate_key(&node_id, &node_id_secret) {
                return Err("node id secret and node id key don't match".to_owned());
            }
        }

        // If we still don't have a valid node id, generate one
        if !node_id.valid || !node_id_secret.valid {
            debug!("generating new node id");
            let (i, s) = key::generate_secret();
            node_id = i;
            node_id_secret = s;
        }
        info!("Node Id is {}", node_id.encode());
        // info!("Node Id Secret is {}", node_id_secret.encode());

        // Save the node id / secret in storage
        protected_store
            .save_user_secret_string("node_id", node_id.encode().as_str())
            .await?;
        protected_store
            .save_user_secret_string("node_id_secret", node_id_secret.encode().as_str())
            .await?;

        self.inner.write().network.node_id = node_id;
        self.inner.write().network.node_id_secret = node_id_secret;

        trace!("init_node_id complete");

        Ok(())
    }
}
