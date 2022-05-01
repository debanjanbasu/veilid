mod bucket;
mod bucket_entry;
mod debug;
mod find_nodes;
mod node_ref;
mod stats_accounting;

use crate::dht::*;
use crate::intf::*;
use crate::network_manager::*;
use crate::rpc_processor::*;
use crate::xx::*;
use crate::*;
use alloc::str::FromStr;
use bucket::*;
pub use bucket_entry::*;
pub use debug::*;
pub use find_nodes::*;
use futures_util::stream::{FuturesUnordered, StreamExt};
pub use node_ref::*;
pub use stats_accounting::*;

//////////////////////////////////////////////////////////////////////////

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Ord, Eq)]
pub enum RoutingDomain {
    PublicInternet,
    LocalNetwork,
}

#[derive(Debug, Default)]
pub struct RoutingDomainDetail {
    dial_info_details: Vec<DialInfoDetail>,
}

struct RoutingTableInner {
    network_manager: NetworkManager,
    node_id: DHTKey,
    node_id_secret: DHTKeySecret,
    buckets: Vec<Bucket>,
    public_internet_routing_domain: RoutingDomainDetail,
    local_network_routing_domain: RoutingDomainDetail,
    bucket_entry_count: usize,

    // Transfer stats for this node
    self_latency_stats_accounting: LatencyStatsAccounting,
    self_transfer_stats_accounting: TransferStatsAccounting,
    self_transfer_stats: TransferStatsDownUp,
}

#[derive(Clone, Debug, Default)]
pub struct RoutingTableHealth {
    pub reliable_entry_count: usize,
    pub unreliable_entry_count: usize,
    pub dead_entry_count: usize,
}

struct RoutingTableUnlockedInner {
    // Background processes
    rolling_transfers_task: TickTask,
    bootstrap_task: TickTask,
    peer_minimum_refresh_task: TickTask,
    ping_validator_task: TickTask,
}

#[derive(Clone)]
pub struct RoutingTable {
    config: VeilidConfig,
    inner: Arc<Mutex<RoutingTableInner>>,
    unlocked_inner: Arc<RoutingTableUnlockedInner>,
}

impl RoutingTable {
    fn new_inner(network_manager: NetworkManager) -> RoutingTableInner {
        RoutingTableInner {
            network_manager,
            node_id: DHTKey::default(),
            node_id_secret: DHTKeySecret::default(),
            buckets: Vec::new(),
            public_internet_routing_domain: RoutingDomainDetail::default(),
            local_network_routing_domain: RoutingDomainDetail::default(),
            bucket_entry_count: 0,
            self_latency_stats_accounting: LatencyStatsAccounting::new(),
            self_transfer_stats_accounting: TransferStatsAccounting::new(),
            self_transfer_stats: TransferStatsDownUp::default(),
        }
    }
    fn new_unlocked_inner(config: VeilidConfig) -> RoutingTableUnlockedInner {
        let c = config.get();
        RoutingTableUnlockedInner {
            rolling_transfers_task: TickTask::new(ROLLING_TRANSFERS_INTERVAL_SECS),
            bootstrap_task: TickTask::new(1),
            peer_minimum_refresh_task: TickTask::new_ms(c.network.dht.min_peer_refresh_time_ms),
            ping_validator_task: TickTask::new(1),
        }
    }
    pub fn new(network_manager: NetworkManager) -> Self {
        let config = network_manager.config();
        let this = Self {
            config: config.clone(),
            inner: Arc::new(Mutex::new(Self::new_inner(network_manager))),
            unlocked_inner: Arc::new(Self::new_unlocked_inner(config)),
        };
        // Set rolling transfers tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .rolling_transfers_task
                .set_routine(move |l, t| {
                    Box::pin(this2.clone().rolling_transfers_task_routine(l, t))
                });
        }
        // Set bootstrap tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .bootstrap_task
                .set_routine(move |_l, _t| Box::pin(this2.clone().bootstrap_task_routine()));
        }
        // Set peer minimum refresh tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .peer_minimum_refresh_task
                .set_routine(move |_l, _t| {
                    Box::pin(this2.clone().peer_minimum_refresh_task_routine())
                });
        }
        // Set ping validator tick task
        {
            let this2 = this.clone();
            this.unlocked_inner
                .ping_validator_task
                .set_routine(move |l, t| Box::pin(this2.clone().ping_validator_task_routine(l, t)));
        }
        this
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.inner.lock().network_manager.clone()
    }
    pub fn rpc_processor(&self) -> RPCProcessor {
        self.network_manager().rpc_processor()
    }

    pub fn node_id(&self) -> DHTKey {
        self.inner.lock().node_id
    }

    pub fn node_id_secret(&self) -> DHTKeySecret {
        self.inner.lock().node_id_secret
    }

    fn with_routing_domain<F, R>(inner: &RoutingTableInner, domain: RoutingDomain, f: F) -> R
    where
        F: FnOnce(&RoutingDomainDetail) -> R,
    {
        match domain {
            RoutingDomain::PublicInternet => f(&inner.public_internet_routing_domain),
            RoutingDomain::LocalNetwork => f(&inner.local_network_routing_domain),
        }
    }

    fn with_routing_domain_mut<F, R>(
        inner: &mut RoutingTableInner,
        domain: RoutingDomain,
        f: F,
    ) -> R
    where
        F: FnOnce(&mut RoutingDomainDetail) -> R,
    {
        match domain {
            RoutingDomain::PublicInternet => f(&mut inner.public_internet_routing_domain),
            RoutingDomain::LocalNetwork => f(&mut inner.local_network_routing_domain),
        }
    }

    pub fn has_dial_info(&self, domain: RoutingDomain) -> bool {
        let inner = self.inner.lock();
        Self::with_routing_domain(&*inner, domain, |rd| !rd.dial_info_details.is_empty())
    }

    pub fn dial_info_details(&self, domain: RoutingDomain) -> Vec<DialInfoDetail> {
        let inner = self.inner.lock();
        Self::with_routing_domain(&*inner, domain, |rd| rd.dial_info_details.clone())
    }

    pub fn first_filtered_dial_info_detail(
        &self,
        domain: Option<RoutingDomain>,
        filter: &DialInfoFilter,
    ) -> Option<DialInfoDetail> {
        let inner = self.inner.lock();
        // Prefer local network first if it isn't filtered out
        if domain == None || domain == Some(RoutingDomain::LocalNetwork) {
            Self::with_routing_domain(&*inner, RoutingDomain::LocalNetwork, |rd| {
                for did in &rd.dial_info_details {
                    if did.matches_filter(filter) {
                        return Some(did.clone());
                    }
                }
                None
            })
        } else {
            None
        }
        .or_else(|| {
            if domain == None || domain == Some(RoutingDomain::PublicInternet) {
                Self::with_routing_domain(&*inner, RoutingDomain::PublicInternet, |rd| {
                    for did in &rd.dial_info_details {
                        if did.matches_filter(filter) {
                            return Some(did.clone());
                        }
                    }
                    None
                })
            } else {
                None
            }
        })
    }

    pub fn all_filtered_dial_info_details(
        &self,
        domain: Option<RoutingDomain>,
        filter: &DialInfoFilter,
    ) -> Vec<DialInfoDetail> {
        let inner = self.inner.lock();
        let mut ret = Vec::new();

        if domain == None || domain == Some(RoutingDomain::LocalNetwork) {
            Self::with_routing_domain(&*inner, RoutingDomain::LocalNetwork, |rd| {
                for did in &rd.dial_info_details {
                    if did.matches_filter(filter) {
                        ret.push(did.clone());
                    }
                }
            });
        }
        if domain == None || domain == Some(RoutingDomain::PublicInternet) {
            Self::with_routing_domain(&*inner, RoutingDomain::PublicInternet, |rd| {
                for did in &rd.dial_info_details {
                    if did.matches_filter(filter) {
                        ret.push(did.clone());
                    }
                }
            });
        }
        ret.remove_duplicates();
        ret
    }

    pub fn register_dial_info(
        &self,
        domain: RoutingDomain,
        dial_info: DialInfo,
        class: DialInfoClass,
    ) -> Result<(), String> {
        trace!(
            "registering dial_info with:\n  domain: {:?}\n  dial_info: {:?}\n  class: {:?}",
            domain,
            dial_info,
            class
        );
        let enable_local_peer_scope = {
            let config = self.network_manager().config();
            let c = config.get();
            c.network.enable_local_peer_scope
        };

        if !enable_local_peer_scope
            && matches!(domain, RoutingDomain::PublicInternet)
            && dial_info.is_local()
        {
            return Err("shouldn't be registering local addresses as public".to_owned())
                .map_err(logthru_rtab!(error));
        }
        if !dial_info.is_valid() {
            return Err(format!(
                "shouldn't be registering invalid addresses: {:?}",
                dial_info
            ))
            .map_err(logthru_rtab!(error));
        }

        let mut inner = self.inner.lock();
        Self::with_routing_domain_mut(&mut *inner, domain, |rd| {
            rd.dial_info_details.push(DialInfoDetail {
                dial_info: dial_info.clone(),
                class,
            });
            rd.dial_info_details.sort();
        });

        let domain_str = match domain {
            RoutingDomain::PublicInternet => "Public",
            RoutingDomain::LocalNetwork => "Local",
        };
        info!(
            "{} Dial Info: {}",
            domain_str,
            NodeDialInfo {
                node_id: NodeId::new(inner.node_id),
                dial_info
            }
            .to_string(),
        );
        debug!("    Class: {:?}", class);
        Ok(())
    }

    pub fn clear_dial_info_details(&self, domain: RoutingDomain) {
        trace!("clearing dial info domain: {:?}", domain);

        let mut inner = self.inner.lock();
        Self::with_routing_domain_mut(&mut *inner, domain, |rd| {
            rd.dial_info_details.clear();
        })
    }

    fn bucket_depth(index: usize) -> usize {
        match index {
            0 => 256,
            1 => 128,
            2 => 64,
            3 => 32,
            4 => 16,
            5 => 8,
            6 => 4,
            7 => 4,
            8 => 4,
            9 => 4,
            _ => 4,
        }
    }

    pub async fn init(&self) -> Result<(), String> {
        let mut inner = self.inner.lock();
        // Size the buckets (one per bit)
        inner.buckets.reserve(DHT_KEY_LENGTH * 8);
        for _ in 0..DHT_KEY_LENGTH * 8 {
            let bucket = Bucket::new(self.clone());
            inner.buckets.push(bucket);
        }

        // make local copy of node id for easy access
        let c = self.config.get();
        inner.node_id = c.network.node_id;
        inner.node_id_secret = c.network.node_id_secret;

        Ok(())
    }

    pub async fn terminate(&self) {
        *self.inner.lock() = Self::new_inner(self.network_manager());
    }

    // Attempt to empty the routing table
    // should only be performed when there are no node_refs (detached)
    pub fn purge(&self) {
        let mut inner = self.inner.lock();
        log_rtab!(
            "Starting routing table purge. Table currently has {} nodes",
            inner.bucket_entry_count
        );
        for bucket in &mut inner.buckets {
            bucket.kick(0);
        }
        log_rtab!(
            "Routing table purge complete. Routing table now has {} nodes",
            inner.bucket_entry_count
        );
    }

    // Attempt to settle buckets and remove entries down to the desired number
    // which may not be possible due extant NodeRefs
    fn kick_bucket(inner: &mut RoutingTableInner, idx: usize) {
        let bucket = &mut inner.buckets[idx];
        let bucket_depth = Self::bucket_depth(idx);

        if let Some(dead_node_ids) = bucket.kick(bucket_depth) {
            // Remove counts
            inner.bucket_entry_count -= dead_node_ids.len();
            log_rtab!("Routing table now has {} nodes", inner.bucket_entry_count);

            // Now purge the routing table inner vectors
            //let filter = |k: &DHTKey| dead_node_ids.contains(k);
            //inner.closest_reliable_nodes.retain(filter);
            //inner.fastest_reliable_nodes.retain(filter);
            //inner.closest_nodes.retain(filter);
            //inner.fastest_nodes.retain(filter);
        }
    }

    fn find_bucket_index(inner: &RoutingTableInner, node_id: DHTKey) -> usize {
        distance(&node_id, &inner.node_id)
            .first_nonzero_bit()
            .unwrap()
    }

    fn drop_node_ref(&self, node_id: DHTKey) {
        // Reduce ref count on entry
        let mut inner = self.inner.lock();
        let idx = Self::find_bucket_index(&*inner, node_id);
        let new_ref_count = {
            let bucket = &mut inner.buckets[idx];
            let entry = bucket.entry_mut(&node_id).unwrap();
            entry.ref_count -= 1;
            entry.ref_count
        };

        // If this entry could possibly go away, kick the bucket
        if new_ref_count == 0 {
            // it important to do this in the same inner lock as the ref count decrease
            Self::kick_bucket(&mut *inner, idx);
        }
    }

    pub fn create_node_ref(&self, node_id: DHTKey) -> Result<NodeRef, String> {
        // Ensure someone isn't trying register this node itself
        if node_id == self.node_id() {
            return Err("can't register own node".to_owned()).map_err(logthru_rtab!(error));
        }

        // Insert into bucket, possibly evicting the newest bucket member
        let noderef = match self.lookup_node_ref(node_id) {
            None => {
                // Make new entry
                let mut inner = self.inner.lock();
                let idx = Self::find_bucket_index(&*inner, node_id);
                let nr = {
                    // Get the bucket for the entry
                    let bucket = &mut inner.buckets[idx];
                    // Add new entry
                    let nr = bucket.add_entry(node_id);

                    // Update count
                    inner.bucket_entry_count += 1;
                    log_rtab!("Routing table now has {} nodes", inner.bucket_entry_count);
                    nr
                };

                // Kick the bucket
                // It is important to do this in the same inner lock as the add_entry
                Self::kick_bucket(&mut *inner, idx);

                nr
            }
            Some(nr) => nr,
        };

        Ok(noderef)
    }

    pub fn lookup_node_ref(&self, node_id: DHTKey) -> Option<NodeRef> {
        let mut inner = self.inner.lock();
        let idx = Self::find_bucket_index(&*inner, node_id);
        let bucket = &mut inner.buckets[idx];
        bucket
            .entry_mut(&node_id)
            .map(|e| NodeRef::new(self.clone(), node_id, e, None))
    }

    // Shortcut function to add a node to our routing table if it doesn't exist
    // and add the dial info we have for it, since that's pretty common
    pub fn register_node_with_node_info(
        &self,
        node_id: DHTKey,
        node_info: NodeInfo,
    ) -> Result<NodeRef, String> {
        let nr = self.create_node_ref(node_id)?;
        nr.operate(move |e| -> Result<(), String> {
            e.update_node_info(node_info);
            Ok(())
        })?;

        Ok(nr)
    }

    // Shortcut function to add a node to our routing table if it doesn't exist
    // and add the last peer address we have for it, since that's pretty common
    pub fn register_node_with_existing_connection(
        &self,
        node_id: DHTKey,
        descriptor: ConnectionDescriptor,
        timestamp: u64,
    ) -> Result<NodeRef, String> {
        let nr = self.create_node_ref(node_id)?;
        nr.operate(move |e| {
            // set the most recent node address for connection finding and udp replies
            e.set_last_connection(descriptor, timestamp);
        });

        Ok(nr)
    }

    fn operate_on_bucket_entry<T, F>(&self, node_id: DHTKey, f: F) -> T
    where
        F: FnOnce(&mut BucketEntry) -> T,
    {
        let mut inner = self.inner.lock();
        let idx = Self::find_bucket_index(&*inner, node_id);
        let bucket = &mut inner.buckets[idx];
        let entry = bucket.entry_mut(&node_id).unwrap();
        f(entry)
    }

    pub fn find_inbound_relay(&self, cur_ts: u64) -> Option<NodeRef> {
        let mut inner = self.inner.lock();
        let mut best_inbound_relay: Option<NodeRef> = None;

        // Iterate all known nodes for candidates
        for b in &mut inner.buckets {
            for (k, entry) in b.entries_mut() {
                // Ensure it's not dead
                if !matches!(entry.state(cur_ts), BucketEntryState::Dead) {
                    // Ensure this node is not on our local network
                    if !entry.local_node_info().has_dial_info() {
                        // Ensure we have the node's status
                        if let Some(node_status) = &entry.peer_stats().status {
                            // Ensure the node will relay
                            if node_status.will_relay {
                                if let Some(best_inbound_relay) = best_inbound_relay.as_mut() {
                                    if best_inbound_relay.operate(|best| {
                                        BucketEntry::cmp_fastest_reliable(cur_ts, best, entry)
                                    }) == std::cmp::Ordering::Greater
                                    {
                                        *best_inbound_relay =
                                            NodeRef::new(self.clone(), *k, entry, None);
                                    }
                                } else {
                                    best_inbound_relay =
                                        Some(NodeRef::new(self.clone(), *k, entry, None));
                                }
                            }
                        }
                    }
                }
            }
        }

        best_inbound_relay
    }

    pub async fn find_self(&self, node_ref: NodeRef) -> Result<Vec<NodeRef>, String> {
        let node_id = self.node_id();
        let rpc_processor = self.rpc_processor();

        let res = rpc_processor
            .clone()
            .rpc_call_find_node(
                Destination::Direct(node_ref.clone()),
                node_id,
                None,
                rpc_processor.make_respond_to_sender(node_ref.clone()),
            )
            .await
            .map_err(map_to_string)
            .map_err(logthru_rtab!())?;
        log_rtab!(
            "find_self for at {:?} answered in {}ms",
            &node_ref,
            timestamp_to_secs(res.latency) * 1000.0f64
        );

        // register nodes we'd found
        self.register_find_node_answer(res)
    }

    pub fn register_find_node_answer(&self, fna: FindNodeAnswer) -> Result<Vec<NodeRef>, String> {
        let node_id = self.node_id();

        // register nodes we'd found
        let mut out = Vec::<NodeRef>::with_capacity(fna.peers.len());
        for p in fna.peers {
            // if our own node if is in the list then ignore it, as we don't add ourselves to our own routing table
            if p.node_id.key == node_id {
                continue;
            }

            // register the node if it's new
            let nr = self
                .register_node_with_node_info(p.node_id.key, p.node_info.clone())
                .map_err(map_to_string)
                .map_err(logthru_rtab!(
                    "couldn't register node {} at {:?}",
                    p.node_id.key,
                    &p.node_info
                ))?;
            out.push(nr);
        }
        Ok(out)
    }

    pub async fn reverse_find_node(&self, node_ref: NodeRef, wide: bool) {
        // Ask bootstrap node to 'find' our own node so we can get some more nodes near ourselves
        // and then contact those nodes to inform -them- that we exist

        // Ask bootstrap server for nodes closest to our own node
        let closest_nodes = match self.find_self(node_ref.clone()).await {
            Err(e) => {
                log_rtab!(error
                    "reverse_find_node: find_self failed for {:?}: {}",
                    &node_ref, e
                );
                return;
            }
            Ok(v) => v,
        };

        // Ask each node near us to find us as well
        if wide {
            for closest_nr in closest_nodes {
                match self.find_self(closest_nr.clone()).await {
                    Err(e) => {
                        log_rtab!(error
                            "reverse_find_node: closest node find_self failed for {:?}: {}",
                            &closest_nr, e
                        );
                        return;
                    }
                    Ok(v) => v,
                };
            }
        }
    }

    async fn bootstrap_task_routine(self) -> Result<(), String> {
        let bootstrap = {
            let c = self.config.get();
            c.network.bootstrap.clone()
        };

        log_rtab!("--- bootstrap_task");

        // Map all bootstrap entries to a single key with multiple dialinfo
        let mut bsmap: BTreeMap<DHTKey, Vec<DialInfoDetail>> = BTreeMap::new();
        for b in bootstrap {
            let ndis = NodeDialInfo::from_str(b.as_str())
                .map_err(map_to_string)
                .map_err(logthru_rtab!("Invalid dial info in bootstrap entry: {}", b))?;
            let node_id = ndis.node_id.key;
            bsmap
                .entry(node_id)
                .or_insert_with(Vec::new)
                .push(DialInfoDetail {
                    dial_info: ndis.dial_info,
                    class: DialInfoClass::Direct, // Bootstraps are always directly reachable
                });
        }
        log_rtab!("    bootstrap list: {:?}", bsmap);

        // Run all bootstrap operations concurrently
        let mut unord = FuturesUnordered::new();
        for (k, v) in bsmap {
            log_rtab!("    bootstrapping {} with {:?}", k.encode(), &v);
            let nr = self
                .register_node_with_node_info(
                    k,
                    NodeInfo {
                        network_class: NetworkClass::InboundCapable, // Bootstraps are always inbound capable
                        outbound_protocols: ProtocolSet::empty(), // Bootstraps do not participate in relaying and will not make outbound requests
                        dial_info_detail_list: v, // Dial info is as specified in the bootstrap list
                        relay_peer_info: None,    // Bootstraps never require a relay themselves
                    },
                )
                .map_err(logthru_rtab!("Couldn't add bootstrap node: {}", k))?;
            unord.push(self.reverse_find_node(nr, true));
        }
        while unord.next().await.is_some() {}
        Ok(())
    }

    ///////////////////////////////////////////////////////////
    /// Peer ping validation

    // Ask our remaining peers to give us more peers before we go
    // back to the bootstrap servers to keep us from bothering them too much
    async fn peer_minimum_refresh_task_routine(self) -> Result<(), String> {
        log_rtab!("--- peer_minimum_refresh task");

        // get list of all peers we know about, even the unreliable ones, and ask them to bootstrap too
        let noderefs = {
            let mut inner = self.inner.lock();
            let mut noderefs = Vec::<NodeRef>::with_capacity(inner.bucket_entry_count);
            for b in &mut inner.buckets {
                for (k, entry) in b.entries_mut() {
                    noderefs.push(NodeRef::new(self.clone(), *k, entry, None))
                }
            }
            noderefs
        };
        log_rtab!("    refreshing with nodes: {:?}", noderefs);

        // do peer minimum search concurrently
        let mut unord = FuturesUnordered::new();
        for nr in noderefs {
            debug!("    --- peer minimum search with {:?}", nr);
            unord.push(self.reverse_find_node(nr, false));
        }
        while unord.next().await.is_some() {}

        Ok(())
    }

    // Ping each node in the routing table if they need to be pinged
    // to determine their reliability
    async fn ping_validator_task_routine(self, _last_ts: u64, cur_ts: u64) -> Result<(), String> {
        log_rtab!("--- ping_validator task");

        let rpc = self.rpc_processor();
        let netman = self.network_manager();
        let relay_node_id = netman.relay_node().map(|nr| nr.node_id());

        let mut inner = self.inner.lock();
        for b in &mut inner.buckets {
            for (k, entry) in b.entries_mut() {
                if entry.needs_ping(k, cur_ts, relay_node_id) {
                    let nr = NodeRef::new(self.clone(), *k, entry, None);
                    log_rtab!(
                        "    --- ping validating: {:?} ({})",
                        nr,
                        entry.state_debug_info(cur_ts)
                    );
                    intf::spawn_local(rpc.clone().rpc_call_info(nr)).detach();
                }
            }
        }
        Ok(())
    }

    // Compute transfer statistics to determine how 'fast' a node is
    async fn rolling_transfers_task_routine(self, last_ts: u64, cur_ts: u64) -> Result<(), String> {
        log_rtab!("--- rolling_transfers task");
        let inner = &mut *self.inner.lock();

        // Roll our own node's transfers
        inner.self_transfer_stats_accounting.roll_transfers(
            last_ts,
            cur_ts,
            &mut inner.self_transfer_stats,
        );

        // Roll all bucket entry transfers
        for b in &mut inner.buckets {
            b.roll_transfers(last_ts, cur_ts);
        }
        Ok(())
    }

    // Ticks about once per second
    // to run tick tasks which may run at slower tick rates as configured
    pub async fn tick(&self) -> Result<(), String> {
        // Do rolling transfers every ROLLING_TRANSFERS_INTERVAL_SECS secs
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // If routing table is empty, then add the bootstrap nodes to it
        if self.inner.lock().bucket_entry_count == 0 {
            self.unlocked_inner.bootstrap_task.tick().await?;
        }

        // If we still don't have enough peers, find nodes until we do
        let min_peer_count = {
            let c = self.config.get();
            c.network.dht.min_peer_count as usize
        };
        if self.inner.lock().bucket_entry_count < min_peer_count {
            self.unlocked_inner.peer_minimum_refresh_task.tick().await?;
        }
        // Ping validate some nodes to groom the table
        self.unlocked_inner.ping_validator_task.tick().await?;

        // Keepalive

        Ok(())
    }

    //////////////////////////////////////////////////////////////////////
    // Stats Accounting
    pub fn stats_question_sent(
        &self,
        node_ref: NodeRef,
        ts: u64,
        bytes: u64,
        expects_answer: bool,
    ) {
        self.inner
            .lock()
            .self_transfer_stats_accounting
            .add_up(bytes);
        node_ref.operate(|e| {
            e.question_sent(ts, bytes, expects_answer);
        })
    }
    pub fn stats_question_rcvd(&self, node_ref: NodeRef, ts: u64, bytes: u64) {
        self.inner
            .lock()
            .self_transfer_stats_accounting
            .add_down(bytes);
        node_ref.operate(|e| {
            e.question_rcvd(ts, bytes);
        })
    }
    pub fn stats_answer_sent(&self, node_ref: NodeRef, ts: u64, bytes: u64) {
        self.inner
            .lock()
            .self_transfer_stats_accounting
            .add_up(bytes);
        node_ref.operate(|e| {
            e.answer_sent(ts, bytes);
        })
    }
    pub fn stats_answer_rcvd(&self, node_ref: NodeRef, send_ts: u64, recv_ts: u64, bytes: u64) {
        self.inner
            .lock()
            .self_transfer_stats_accounting
            .add_down(bytes);
        self.inner
            .lock()
            .self_latency_stats_accounting
            .record_latency(recv_ts - send_ts);
        node_ref.operate(|e| {
            e.answer_rcvd(send_ts, recv_ts, bytes);
        })
    }
    pub fn stats_question_lost(&self, node_ref: NodeRef, ts: u64) {
        node_ref.operate(|e| {
            e.question_lost(ts);
        })
    }

    //////////////////////////////////////////////////////////////////////
    // Routing Table Health Metrics

    pub fn get_routing_table_health(&self) -> RoutingTableHealth {
        let mut health = RoutingTableHealth::default();
        let cur_ts = intf::get_timestamp();
        let inner = self.inner.lock();
        for bucket in &inner.buckets {
            for entry in bucket.entries() {
                match entry.1.state(cur_ts) {
                    BucketEntryState::Reliable => {
                        health.reliable_entry_count += 1;
                    }
                    BucketEntryState::Unreliable => {
                        health.unreliable_entry_count += 1;
                    }
                    BucketEntryState::Dead => {
                        health.dead_entry_count += 1;
                    }
                }
            }
        }
        health
    }
}
