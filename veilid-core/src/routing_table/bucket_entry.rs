use super::*;
use core::sync::atomic::{AtomicU32, Ordering};

// Reliable pings are done with increased spacing between pings
// - Start secs is the number of seconds between the first two pings
// - Max secs is the maximum number of seconds between consecutive pings
// - Multiplier changes the number of seconds between pings over time
//   making it longer as the node becomes more reliable
const RELIABLE_PING_INTERVAL_START_SECS: u32 = 10;
const RELIABLE_PING_INTERVAL_MAX_SECS: u32 = 10 * 60;
const RELIABLE_PING_INTERVAL_MULTIPLIER: f64 = 2.0;

// Unreliable pings are done for a fixed amount of time while the
// node is given a chance to come back online before it is made dead
// If a node misses a single ping, it is marked unreliable and must
// return reliable pings for the duration of the span before being
// marked reliable again
// - Span is the number of seconds total to attempt to validate the node
// - Interval is the number of seconds between each ping
const UNRELIABLE_PING_SPAN_SECS: u32 = 60;
const UNRELIABLE_PING_INTERVAL_SECS: u32 = 5;

// Keepalive pings are done occasionally to ensure holepunched public dialinfo
// remains valid, as well as to make sure we remain in any relay node's routing table
const KEEPALIVE_PING_INTERVAL_SECS: u32 = 20;

// How many times do we try to ping a never-reached node before we call it dead
const NEVER_REACHED_PING_COUNT: u32 = 3;

// Do not change order here, it will mess up other sorts
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum BucketEntryState {
    Dead,
    Unreliable,
    Reliable,
}

#[derive(Debug)]
pub struct BucketEntryInner {
    min_max_version: Option<(u8, u8)>,
    seen_our_node_info: bool,
    last_connection: Option<(ConnectionDescriptor, u64)>,
    opt_signed_node_info: Option<SignedNodeInfo>,
    opt_local_node_info: Option<LocalNodeInfo>,
    peer_stats: PeerStats,
    latency_stats_accounting: LatencyStatsAccounting,
    transfer_stats_accounting: TransferStatsAccounting,
    #[cfg(feature = "tracking")]
    next_track_id: usize,
    #[cfg(feature = "tracking")]
    node_ref_tracks: HashMap<usize, backtrace::Backtrace>,
}

impl BucketEntryInner {
    #[cfg(feature = "tracking")]
    pub fn track(&mut self) -> usize {
        let track_id = self.next_track_id;
        self.next_track_id += 1;
        self.node_ref_tracks
            .insert(track_id, backtrace::Backtrace::new_unresolved());
        track_id
    }

    #[cfg(feature = "tracking")]
    pub fn untrack(&mut self, track_id: usize) {
        self.node_ref_tracks.remove(&track_id);
    }

    // Less is faster
    pub fn cmp_fastest(e1: &Self, e2: &Self) -> std::cmp::Ordering {
        // Lower latency to the front
        if let Some(e1_latency) = &e1.peer_stats.latency {
            if let Some(e2_latency) = &e2.peer_stats.latency {
                e1_latency.average.cmp(&e2_latency.average)
            } else {
                std::cmp::Ordering::Less
            }
        } else if e2.peer_stats.latency.is_some() {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    }

    // Less is more reliable then faster
    pub fn cmp_fastest_reliable(cur_ts: u64, e1: &Self, e2: &Self) -> std::cmp::Ordering {
        // Reverse compare so most reliable is at front
        let ret = e2.state(cur_ts).cmp(&e1.state(cur_ts));
        if ret != std::cmp::Ordering::Equal {
            return ret;
        }

        // Lower latency to the front
        if let Some(e1_latency) = &e1.peer_stats.latency {
            if let Some(e2_latency) = &e2.peer_stats.latency {
                e1_latency.average.cmp(&e2_latency.average)
            } else {
                std::cmp::Ordering::Less
            }
        } else if e2.peer_stats.latency.is_some() {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    }

    pub fn sort_fastest_reliable_fn(cur_ts: u64) -> impl FnMut(&Self, &Self) -> std::cmp::Ordering {
        move |e1, e2| Self::cmp_fastest_reliable(cur_ts, e1, e2)
    }

    pub fn update_node_info(&mut self, signed_node_info: SignedNodeInfo) {
        // Don't update with older node info, or something less valid
        if let Some(current_sni) = &self.opt_signed_node_info {
            if current_sni.signature.valid && !signed_node_info.signature.valid {
                return;
            }
            if signed_node_info.timestamp < current_sni.timestamp {
                return;
            }
        }
        self.min_max_version = Some((
            signed_node_info.node_info.min_version,
            signed_node_info.node_info.max_version,
        ));
        self.opt_signed_node_info = Some(signed_node_info);
    }
    pub fn update_local_node_info(&mut self, local_node_info: LocalNodeInfo) {
        self.opt_local_node_info = Some(local_node_info)
    }

    pub fn has_node_info(&self) -> bool {
        self.opt_signed_node_info.is_some()
    }

    pub fn has_valid_signed_node_info(&self) -> bool {
        if let Some(sni) = &self.opt_signed_node_info {
            sni.is_valid()
        } else {
            false
        }
    }

    pub fn has_local_node_info(&self) -> bool {
        self.opt_local_node_info.is_some()
    }

    pub fn node_info(&self) -> Option<NodeInfo> {
        self.opt_signed_node_info
            .as_ref()
            .map(|s| s.node_info.clone())
    }
    pub fn local_node_info(&self) -> Option<LocalNodeInfo> {
        self.opt_local_node_info.clone()
    }
    pub fn peer_info(&self, key: DHTKey) -> Option<PeerInfo> {
        self.opt_signed_node_info.as_ref().map(|s| PeerInfo {
            node_id: NodeId::new(key),
            signed_node_info: s.clone(),
        })
    }

    pub fn set_last_connection(&mut self, last_connection: ConnectionDescriptor, timestamp: u64) {
        self.last_connection = Some((last_connection, timestamp));
    }

    pub fn last_connection(&self) -> Option<(ConnectionDescriptor, u64)> {
        self.last_connection
    }

    pub fn set_min_max_version(&mut self, min_max_version: (u8, u8)) {
        self.min_max_version = Some(min_max_version);
    }

    pub fn min_max_version(&self) -> Option<(u8, u8)> {
        self.min_max_version
    }

    pub fn state(&self, cur_ts: u64) -> BucketEntryState {
        if self.check_reliable(cur_ts) {
            BucketEntryState::Reliable
        } else if self.check_dead(cur_ts) {
            BucketEntryState::Dead
        } else {
            BucketEntryState::Unreliable
        }
    }

    pub fn peer_stats(&self) -> &PeerStats {
        &self.peer_stats
    }

    pub fn update_node_status(&mut self, status: NodeStatus) {
        self.peer_stats.status = Some(status);
    }

    pub fn set_seen_our_node_info(&mut self, seen: bool) {
        self.seen_our_node_info = seen;
    }

    pub fn has_seen_our_node_info(&self) -> bool {
        self.seen_our_node_info
    }

    ///// stats methods
    // called every ROLLING_TRANSFERS_INTERVAL_SECS seconds
    pub(super) fn roll_transfers(&mut self, last_ts: u64, cur_ts: u64) {
        self.transfer_stats_accounting.roll_transfers(
            last_ts,
            cur_ts,
            &mut self.peer_stats.transfer,
        );
    }

    // Called for every round trip packet we receive
    fn record_latency(&mut self, latency: u64) {
        self.peer_stats.latency = Some(self.latency_stats_accounting.record_latency(latency));
    }

    ///// state machine handling
    pub(super) fn check_reliable(&self, cur_ts: u64) -> bool {
        // If we have had any failures to send, this is not reliable
        if self.peer_stats.rpc_stats.failed_to_send > 0 {
            return false;
        }

        // if we have seen the node consistently for longer that UNRELIABLE_PING_SPAN_SECS
        match self.peer_stats.rpc_stats.first_consecutive_seen_ts {
            None => false,
            Some(ts) => {
                cur_ts.saturating_sub(ts) >= (UNRELIABLE_PING_SPAN_SECS as u64 * 1000000u64)
            }
        }
    }
    pub(super) fn check_dead(&self, cur_ts: u64) -> bool {
        // If we have failured to send NEVER_REACHED_PING_COUNT times in a row, the node is dead
        if self.peer_stats.rpc_stats.failed_to_send >= NEVER_REACHED_PING_COUNT {
            return true;
        }
        // if we have not heard from the node at all for the duration of the unreliable ping span
        // a node is not dead if we haven't heard from it yet,
        // but we give it NEVER_REACHED_PING_COUNT chances to ping before we say it's dead
        match self.peer_stats.rpc_stats.last_seen_ts {
            None => self.peer_stats.rpc_stats.recent_lost_answers < NEVER_REACHED_PING_COUNT,
            Some(ts) => {
                cur_ts.saturating_sub(ts) >= (UNRELIABLE_PING_SPAN_SECS as u64 * 1000000u64)
            }
        }
    }

    fn needs_constant_ping(&self, cur_ts: u64, interval: u64) -> bool {
        // If we have not either seen the node, nor asked it a question in the last 'interval'
        // then we should ping it
        let latest_contact_time = self
            .peer_stats
            .rpc_stats
            .last_seen_ts
            .max(self.peer_stats.rpc_stats.last_question);

        match latest_contact_time {
            None => true,
            Some(latest_contact_time) => {
                // If we haven't done anything with this node in 'interval' seconds
                cur_ts.saturating_sub(latest_contact_time) >= (interval * 1000000u64)
            }
        }
    }

    // Check if this node needs a ping right now to validate it is still reachable
    pub(super) fn needs_ping(
        &self,
        node_id: &DHTKey,
        cur_ts: u64,
        relay_node_id: Option<DHTKey>,
    ) -> bool {
        // See which ping pattern we are to use
        let state = self.state(cur_ts);

        // If this entry is our relay node, then we should ping it regularly to keep our association alive
        if let Some(relay_node_id) = relay_node_id {
            if relay_node_id == *node_id {
                return self.needs_constant_ping(cur_ts, KEEPALIVE_PING_INTERVAL_SECS as u64);
            }
        }

        match state {
            BucketEntryState::Reliable => {
                // If we are in a reliable state, we need a ping on an exponential scale
                let latest_contact_time = self
                    .peer_stats
                    .rpc_stats
                    .last_seen_ts
                    .max(self.peer_stats.rpc_stats.last_question);

                match latest_contact_time {
                    None => {
                        error!("Peer is reliable, but not seen!");
                        true
                    }
                    Some(latest_contact_time) => {
                        let first_consecutive_seen_ts =
                            self.peer_stats.rpc_stats.first_consecutive_seen_ts.unwrap();
                        let start_of_reliable_time = first_consecutive_seen_ts
                            + ((UNRELIABLE_PING_SPAN_SECS - UNRELIABLE_PING_INTERVAL_SECS) as u64
                                * 1_000_000u64);
                        let reliable_cur = cur_ts.saturating_sub(start_of_reliable_time);
                        let reliable_last =
                            latest_contact_time.saturating_sub(start_of_reliable_time);

                        retry_falloff_log(
                            reliable_last,
                            reliable_cur,
                            RELIABLE_PING_INTERVAL_START_SECS as u64 * 1_000_000u64,
                            RELIABLE_PING_INTERVAL_MAX_SECS as u64 * 1_000_000u64,
                            RELIABLE_PING_INTERVAL_MULTIPLIER,
                        )
                    }
                }
            }
            BucketEntryState::Unreliable => {
                // If we are in an unreliable state, we need a ping every UNRELIABLE_PING_INTERVAL_SECS seconds
                self.needs_constant_ping(cur_ts, UNRELIABLE_PING_INTERVAL_SECS as u64)
            }
            BucketEntryState::Dead => false,
        }
    }

    pub(super) fn touch_last_seen(&mut self, ts: u64) {
        // Mark the node as seen
        if self
            .peer_stats
            .rpc_stats
            .first_consecutive_seen_ts
            .is_none()
        {
            self.peer_stats.rpc_stats.first_consecutive_seen_ts = Some(ts);
        }

        self.peer_stats.rpc_stats.last_seen_ts = Some(ts);
    }

    pub(super) fn _state_debug_info(&self, cur_ts: u64) -> String {
        let first_consecutive_seen_ts = if let Some(first_consecutive_seen_ts) =
            self.peer_stats.rpc_stats.first_consecutive_seen_ts
        {
            format!(
                "{}s ago",
                timestamp_to_secs(cur_ts.saturating_sub(first_consecutive_seen_ts))
            )
        } else {
            "never".to_owned()
        };
        let last_seen_ts_str = if let Some(last_seen_ts) = self.peer_stats.rpc_stats.last_seen_ts {
            format!(
                "{}s ago",
                timestamp_to_secs(cur_ts.saturating_sub(last_seen_ts))
            )
        } else {
            "never".to_owned()
        };

        format!(
            "state: {:?}, first_consecutive_seen_ts: {}, last_seen_ts: {}",
            self.state(cur_ts),
            first_consecutive_seen_ts,
            last_seen_ts_str
        )
    }

    ////////////////////////////////////////////////////////////////
    /// Called when rpc processor things happen

    pub(super) fn question_sent(&mut self, ts: u64, bytes: u64, expects_answer: bool) {
        self.transfer_stats_accounting.add_up(bytes);
        self.peer_stats.rpc_stats.messages_sent += 1;
        self.peer_stats.rpc_stats.failed_to_send = 0;
        if expects_answer {
            self.peer_stats.rpc_stats.questions_in_flight += 1;
            self.peer_stats.rpc_stats.last_question = Some(ts);
        }
    }
    pub(super) fn question_rcvd(&mut self, ts: u64, bytes: u64) {
        self.transfer_stats_accounting.add_down(bytes);
        self.peer_stats.rpc_stats.messages_rcvd += 1;
        self.touch_last_seen(ts);
    }
    pub(super) fn answer_sent(&mut self, bytes: u64) {
        self.transfer_stats_accounting.add_up(bytes);
        self.peer_stats.rpc_stats.messages_sent += 1;
        self.peer_stats.rpc_stats.failed_to_send = 0;
    }
    pub(super) fn answer_rcvd(&mut self, send_ts: u64, recv_ts: u64, bytes: u64) {
        self.transfer_stats_accounting.add_down(bytes);
        self.peer_stats.rpc_stats.messages_rcvd += 1;
        self.peer_stats.rpc_stats.questions_in_flight -= 1;
        self.record_latency(recv_ts - send_ts);
        self.touch_last_seen(recv_ts);
        self.peer_stats.rpc_stats.recent_lost_answers = 0;
    }
    pub(super) fn question_lost(&mut self) {
        self.peer_stats.rpc_stats.first_consecutive_seen_ts = None;
        self.peer_stats.rpc_stats.questions_in_flight -= 1;
        self.peer_stats.rpc_stats.recent_lost_answers += 1;
    }
    pub(super) fn failed_to_send(&mut self, ts: u64, expects_answer: bool) {
        if expects_answer {
            self.peer_stats.rpc_stats.last_question = Some(ts);
        }
        self.peer_stats.rpc_stats.failed_to_send += 1;
        self.peer_stats.rpc_stats.first_consecutive_seen_ts = None;
    }
}

#[derive(Debug)]
pub struct BucketEntry {
    pub(super) ref_count: AtomicU32,
    inner: RwLock<BucketEntryInner>,
}

impl BucketEntry {
    pub(super) fn new() -> Self {
        let now = get_timestamp();
        Self {
            ref_count: AtomicU32::new(0),
            inner: RwLock::new(BucketEntryInner {
                min_max_version: None,
                seen_our_node_info: false,
                last_connection: None,
                opt_signed_node_info: None,
                opt_local_node_info: None,
                peer_stats: PeerStats {
                    time_added: now,
                    rpc_stats: RPCStats::default(),
                    latency: None,
                    transfer: TransferStatsDownUp::default(),
                    status: None,
                },
                latency_stats_accounting: LatencyStatsAccounting::new(),
                transfer_stats_accounting: TransferStatsAccounting::new(),
                #[cfg(feature = "tracking")]
                next_track_id: 0,
                #[cfg(feature = "tracking")]
                node_ref_tracks: HashMap::new(),
            }),
        }
    }

    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&BucketEntryInner) -> R,
    {
        let inner = self.inner.read();
        f(&*inner)
    }

    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut BucketEntryInner) -> R,
    {
        let mut inner = self.inner.write();
        f(&mut *inner)
    }
}

impl Drop for BucketEntry {
    fn drop(&mut self) {
        if self.ref_count.load(Ordering::Relaxed) != 0 {
            #[cfg(feature = "tracking")]
            {
                println!("NodeRef Tracking");
                for (id, bt) in &mut self.node_ref_tracks {
                    bt.resolve();
                    println!("Id: {}\n----------------\n{:#?}", id, bt);
                }
            }

            panic!(
                "bucket entry dropped with non-zero refcount: {:#?}",
                self.inner.read().node_info()
            )
        }
    }
}
