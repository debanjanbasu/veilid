use super::*;
use crate::veilid_api::*;
use serde::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RouteSpecDetail {
    /// Secret key
    #[serde(skip)]
    secret_key: DHTKeySecret,
    /// Route hops
    hops: Vec<DHTKey>,
    /// Route noderefs
    #[serde(skip)]
    hop_node_refs: Vec<NodeRef>,
    /// Transfers up and down
    transfer_stats_down_up: TransferStatsDownUp,
    /// Latency stats
    latency_stats: LatencyStats,
    /// Accounting mechanism for this route's RPC latency
    #[serde(skip)]
    latency_stats_accounting: LatencyStatsAccounting,
    /// Accounting mechanism for the bandwidth across this route
    #[serde(skip)]
    transfer_stats_accounting: TransferStatsAccounting,
    /// Published private route, do not reuse for ephemeral routes
    /// Not serialized because all routes should be re-published when restarting
    #[serde(skip)]
    published: bool,
    /// Timestamp of when the route was created
    created_ts: u64,
    /// Timestamp of when the route was last checked for validity
    last_checked_ts: Option<u64>,
    /// Directions this route is guaranteed to work in
    directions: DirectionSet,
}

/// The core representation of the RouteSpecStore that can be serialized
#[derive(Debug, Serialize, Deserialize)]
pub struct RouteSpecStoreContent {
    /// All of the routes we have allocated so far
    details: HashMap<DHTKey, RouteSpecDetail>,
}

/// Ephemeral data used to help the RouteSpecStore operate efficiently
#[derive(Debug, Default)]
pub struct RouteSpecStoreCache {
    /// How many times nodes have been used
    used_nodes: HashMap<DHTKey, usize>,
    /// How many times nodes have been used at the terminal point of a route
    used_end_nodes: HashMap<DHTKey, usize>,
    /// Route spec hop cache, used to quickly disqualify routes
    hop_cache: HashSet<Vec<u8>>,
}

#[derive(Debug)]
pub struct RouteSpecStore {
    /// Serialize RouteSpecStore content
    content: RouteSpecStoreContent,
    /// RouteSpecStore cache
    cache: RouteSpecStoreCache,
}

fn route_spec_to_hop_cache(spec: Arc<RouteSpec>) -> Vec<u8> {
    let mut cache: Vec<u8> = Vec::with_capacity(spec.hops.len() * DHT_KEY_LENGTH);
    for hop in spec.hops {
        cache.extend_from_slice(&hop.dial_info.node_id.key.bytes);
    }
    cache
}

/// number of route permutations is the number of unique orderings
/// for a set of nodes, given that the first node is fixed
fn get_route_permutation_count(hop_count: usize) -> usize {
    if hop_count == 0 {
        unreachable!();
    }
    // a single node or two nodes is always fixed
    if hop_count == 1 || hop_count == 2 {
        return 1;
    }
    // more than two nodes has factorial permutation
    // hop_count = 3 -> 2! -> 2
    // hop_count = 4 -> 3! -> 6
    (3..hop_count).into_iter().fold(2usize, |acc, x| acc * x)
}

/// get the route permutation at particular 'perm' index, starting at the 'start' index
/// for a set of 'hop_count' nodes. the first node is always fixed, and the maximum
/// number of permutations is given by get_route_permutation_count()
fn with_route_permutations<F>(hop_count: usize, start: usize, f: F) -> bool
where
    F: FnMut(&[usize]) -> bool,
{
    if hop_count == 0 {
        unreachable!();
    }
    // initial permutation
    let mut permutation: Vec<usize> = Vec::with_capacity(hop_count);
    for n in 0..hop_count {
        permutation[n] = start + n;
    }
    // if we have one hop or two, then there's only one permutation
    if hop_count == 1 || hop_count == 2 {
        return f(&permutation);
    }

    // heaps algorithm
    fn heaps_permutation<F>(permutation: &mut [usize], size: usize, f: F) -> bool
    where
        F: FnMut(&[usize]) -> bool,
    {
        if size == 1 {
            if f(&permutation) {
                return true;
            }
            return false;
        }

        for i in 0..size {
            if heaps_permutation(permutation, size - 1, f) {
                return true;
            }
            if size % 2 == 1 {
                permutation.swap(1, size);
            } else {
                permutation.swap(1 + i, size);
            }
        }
        false
    }

    // recurse
    heaps_permutation(&mut permutation, hop_count - 1, f)
}

/// get the hop cache key for a particular route permutation
fn route_permutation_to_hop_cache(nodes: &[(DHTKey, NodeInfo)], perm: &[usize]) -> Vec<u8> {
    let mut cache: Vec<u8> = Vec::with_capacity(perm.len() * DHT_KEY_LENGTH);
    for n in perm {
        cache.extend_from_slice(&nodes[*n].0.bytes)
    }
    cache
}

impl RouteSpecStore {
    pub fn new() -> Self {
        Self {
            content: RouteSpecStoreContent {
                details: HashMap::new(),
            },
            cache: Default::default(),
        }
    }

    pub fn load(routing_table: RoutingTable) -> Result<RouteSpecStore, VeilidAPIError> {
        // Get cbor blob from table store
        let content: RouteSpecStoreContent = serde_cbor::from_slice(cbor)
            .map_err(|e| VeilidAPIError::parse_error("invalid route spec store content", e))?;
        let rss = RouteSpecStore {
            content,
            cache: Default::default(),
        };
        rss.rebuild_cache(routing_table);
        Ok(rss)
    }

    pub fn save(&self, routing_table: RoutingTable) -> Result<(), VeilidAPIError> {
        // Save all the fields we care about to the cbor blob in table storage
        let cbor = serde_cbor::to_vec(&self.content).unwrap();
        let table_store = routing_table.network_manager().table_store();
        table_store.open("")
    }

    fn rebuild_cache(&mut self, routing_table: RoutingTable) {
        //
        // xxx also load secrets from pstore
        let pstore = routing_table.network_manager().protected_store();
    }

    fn detail_mut(&mut self, public_key: DHTKey) -> &mut RouteSpecDetail {
        self.content.details.get_mut(&public_key).unwrap()
    }

    /// Create a new route
    /// Prefers nodes that are not currently in use by another route
    /// The route is not yet tested for its reachability
    /// Returns None if no route could be allocated at this time
    pub fn allocate_route(
        &mut self,
        routing_table: RoutingTable,
        reliable: bool,
        hop_count: usize,
        directions: DirectionSet,
    ) -> Option<DHTKey> {
        use core::cmp::Ordering;

        let max_route_hop_count = {
            let config = routing_table.network_manager().config();
            let c = config.get();
            let max_route_hop_count = c.network.rpc.max_route_hop_count;
            max_route_hop_count.into()
        };

        if hop_count < 2 {
            log_rtab!(error "Not allocating route less than two hops in length");
            return None;
        }

        if hop_count > max_route_hop_count {
            log_rtab!(error "Not allocating route longer than max route hop count");
            return None;
        }

        // Get list of all nodes, and sort them for selection
        let cur_ts = intf::get_timestamp();
        let dial_info_sort = if reliable {
            Some(DialInfoDetail::reliable_sort)
        } else {
            None
        };
        let filter = |rti, k: DHTKey, v: Option<Arc<BucketEntry>>| -> bool {
            // Exclude our own node from routes
            if v.is_none() {
                return false;
            }
            let v = v.unwrap();

            // Exclude nodes on our local network
            let on_local_network = v.with(rti, |_rti, e| {
                e.node_info(RoutingDomain::LocalNetwork).is_some()
            });
            if on_local_network {
                return false;
            }

            // Exclude nodes with no publicinternet nodeinfo, or incompatible nodeinfo or node status won't route
            v.with(rti, |_rti, e| {
                let node_info_ok = if let Some(ni) = e.node_info(RoutingDomain::PublicInternet) {
                    ni.has_any_dial_info()
                } else {
                    false
                };
                let node_status_ok = if let Some(ns) = e.node_status(RoutingDomain::PublicInternet)
                {
                    ns.will_route()
                } else {
                    false
                };

                node_info_ok && node_status_ok
            })
        };
        let compare = |rti,
                       v1: &(DHTKey, Option<Arc<BucketEntry>>),
                       v2: &(DHTKey, Option<Arc<BucketEntry>>)|
         -> Ordering {
            // deprioritize nodes that we have already used as end points
            let e1_used_end = self
                .cache
                .used_end_nodes
                .get(&v1.0)
                .cloned()
                .unwrap_or_default();
            let e2_used_end = self
                .cache
                .used_end_nodes
                .get(&v2.0)
                .cloned()
                .unwrap_or_default();
            let cmp_used_end = e1_used_end.cmp(&e2_used_end);
            if !matches!(cmp_used_end, Ordering::Equal) {
                return cmp_used_end;
            }

            // deprioritize nodes we have used already anywhere
            let e1_used = self
                .cache
                .used_nodes
                .get(&v1.0)
                .cloned()
                .unwrap_or_default();
            let e2_used = self
                .cache
                .used_nodes
                .get(&v2.0)
                .cloned()
                .unwrap_or_default();
            let cmp_used = e1_used.cmp(&e2_used);
            if !matches!(cmp_used, Ordering::Equal) {
                return cmp_used;
            }

            // always prioritize reliable nodes, but sort by oldest or fastest
            let cmpout = v1.1.unwrap().with(rti, |rti, e1| {
                v2.1.unwrap().with(rti, |_rti, e2| {
                    if reliable {
                        BucketEntryInner::cmp_oldest_reliable(cur_ts, e1, e2)
                    } else {
                        BucketEntryInner::cmp_fastest_reliable(cur_ts, e1, e2)
                    }
                })
            });
            cmpout
        };
        let transform = |rti, k: DHTKey, v: Option<Arc<BucketEntry>>| -> (DHTKey, NodeInfo) {
            // Return the key and the nodeinfo for that key
            (
                k,
                v.unwrap().with(rti, |_rti, e| {
                    e.node_info(RoutingDomain::PublicInternet.into())
                        .unwrap()
                        .clone()
                }),
            )
        };

        // Pull the whole routing table in sorted order
        let node_count = routing_table.get_entry_count(
            RoutingDomain::PublicInternet.into(),
            BucketEntryState::Unreliable,
        );
        let mut nodes = routing_table
            .find_peers_with_sort_and_filter(node_count, cur_ts, filter, compare, transform);

        // If we couldn't find enough nodes, wait until we have more nodes in the routing table
        if nodes.len() < hop_count {
            log_rtab!(debug "Not enough nodes to construct route at this time. Try again later.");
            return None;
        }

        // Now go through nodes and try to build a route we haven't seen yet
        let mut route_nodes: Vec<usize> = Vec::with_capacity(hop_count);
        for start in 0..(nodes.len() - hop_count) {
            // Try the permutations available starting with 'start'
            let done = with_route_permutations(hop_count, start, |permutation: &[usize]| {
                // Get the route cache key
                let key = route_permutation_to_hop_cache(&nodes, permutation);

                // Skip routes we have already seen
                if self.cache.hop_cache.contains(&key) {
                    return false;
                }

                // Ensure this route is viable by checking that each node can contact the next one
                if directions.contains(Direction::Outbound) {
                    let our_node_info =
                        routing_table.get_own_node_info(RoutingDomain::PublicInternet);
                    let mut previous_node_info = &our_node_info;
                    let mut reachable = true;
                    for n in permutation {
                        let current_node_info = &nodes.get(*n).as_ref().unwrap().1;
                        let cm = NetworkManager::get_node_contact_method(
                            previous_node_info,
                            current_node_info,
                        );
                        if matches!(cm, ContactMethod::Unreachable) {
                            reachable = false;
                            break;
                        }
                        previous_node_info = current_node_info;
                    }
                    if !reachable {
                        return false;
                    }
                }
                if directions.contains(Direction::Inbound) {
                    let our_node_info =
                        routing_table.get_own_node_info(RoutingDomain::PublicInternet);
                    let mut next_node_info = &our_node_info;
                    let mut reachable = true;
                    for n in permutation.iter().rev() {
                        let current_node_info = &nodes.get(*n).as_ref().unwrap().1;
                        let cm = NetworkManager::get_node_contact_method(
                            current_node_info,
                            next_node_info,
                        );
                        if matches!(cm, ContactMethod::Unreachable) {
                            reachable = false;
                            break;
                        }
                        next_node_info = current_node_info;
                    }
                    if !reachable {
                        return false;
                    }
                }
                // Keep this route
                route_nodes = permutation.to_vec();
                true
            });
            if done {
                break;
            }
        }
        if route_nodes.is_empty() {
            return None;
        }

        // Got a unique route, lets build the detail, register it, and return it
        let hops = route_nodes.iter().map(|v| nodes[*v].0).collect();
        let hop_node_refs = route_nodes
            .iter()
            .map(|v| routing_table.lookup_node_ref(nodes[*v].0).unwrap())
            .collect();

        let (public_key, secret_key) = generate_secret();

        let rsd = RouteSpecDetail {
            secret_key,
            hops,
            hop_node_refs,
            transfer_stats_down_up: Default::default(),
            latency_stats: Default::default(),
            latency_stats_accounting: Default::default(),
            transfer_stats_accounting: Default::default(),
            published: false,
            created_ts: cur_ts,
            last_checked_ts: None,
            directions,
        };

        self.content.details.insert(public_key, rsd);

        // xxx insert into cache too

        Some(public_key)
    }

    pub fn release_route(&mut self, spec: Arc<RouteSpec>) {}

    pub fn best_route(&mut self, reliable: bool) -> Arc<RouteSpec> {}

    /// Mark route as published
    /// When first deserialized, routes must be re-published in order to ensure they remain
    /// in the RouteSpecStore.
    pub fn mark_route_published(&mut self, spec: Arc<RouteSpec>) {
        self.detail_mut(spec).published = true;
    }

    /// Mark route as checked
    pub fn touch_route_checked(&mut self, spec: Arc<RouteSpec>, cur_ts: u64) {
        self.detail_mut(spec).last_checked_ts = cur_ts;
    }

    pub fn record_latency(&mut self, spec: Arc<RouteSpec>, latency: u64) {
        let lsa = self.detail_mut(spec).latency_stats_accounting;
        self.detail_mut(spec).latency_stats = lsa.record_latency(latency);
    }

    pub fn latency_stats(&self, spec: Arc<RouteSpec>) -> LatencyStats {
        self.detail_mut(spec).latency_stats.clone()
    }

    pub fn add_down(&mut self, spec: Arc<RouteSpec>, bytes: u64) {
        self.current_transfer.down += bytes;
    }

    pub fn add_up(&mut self, spec: Arc<RouteSpec>, bytes: u64) {}

    pub fn roll_transfers(&mut self) {
        //
    }
}
