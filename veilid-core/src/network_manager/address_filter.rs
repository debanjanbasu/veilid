use super::*;
use alloc::collections::btree_map::Entry;

// XXX: Move to config eventually?
const PUNISHMENT_DURATION_MIN: usize = 60;

#[derive(ThisError, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressFilterError {
    #[error("Count exceeded")]
    CountExceeded,
    #[error("Rate exceeded")]
    RateExceeded,
    #[error("Address is punished")]
    Punished,
}

#[derive(ThisError, Debug, Clone, Copy, PartialEq, Eq)]
#[error("Address not in table")]
pub struct AddressNotInTableError {}

#[derive(Debug)]
struct AddressFilterInner {
    conn_count_by_ip4: BTreeMap<Ipv4Addr, usize>,
    conn_count_by_ip6_prefix: BTreeMap<Ipv6Addr, usize>,
    conn_timestamps_by_ip4: BTreeMap<Ipv4Addr, Vec<Timestamp>>,
    conn_timestamps_by_ip6_prefix: BTreeMap<Ipv6Addr, Vec<Timestamp>>,
    punishments_by_ip4: BTreeMap<Ipv4Addr, Timestamp>,
    punishments_by_ip6_prefix: BTreeMap<Ipv6Addr, Timestamp>,
}

#[derive(Debug)]
struct AddressFilterUnlockedInner {
    max_connections_per_ip4: usize,
    max_connections_per_ip6_prefix: usize,
    max_connections_per_ip6_prefix_size: usize,
    max_connection_frequency_per_min: usize,
    punishment_duration_min: usize,
}

#[derive(Clone, Debug)]
pub struct AddressFilter {
    unlocked_inner: Arc<AddressFilterUnlockedInner>,
    inner: Arc<Mutex<AddressFilterInner>>,
}

impl AddressFilter {
    pub fn new(config: VeilidConfig) -> Self {
        let c = config.get();
        Self {
            unlocked_inner: Arc::new(AddressFilterUnlockedInner {
                max_connections_per_ip4: c.network.max_connections_per_ip4 as usize,
                max_connections_per_ip6_prefix: c.network.max_connections_per_ip6_prefix as usize,
                max_connections_per_ip6_prefix_size: c.network.max_connections_per_ip6_prefix_size
                    as usize,
                max_connection_frequency_per_min: c.network.max_connection_frequency_per_min
                    as usize,
                punishment_duration_min: PUNISHMENT_DURATION_MIN,
            }),
            inner: Arc::new(Mutex::new(AddressFilterInner {
                conn_count_by_ip4: BTreeMap::new(),
                conn_count_by_ip6_prefix: BTreeMap::new(),
                conn_timestamps_by_ip4: BTreeMap::new(),
                conn_timestamps_by_ip6_prefix: BTreeMap::new(),
                punishments_by_ip4: BTreeMap::new(),
                punishments_by_ip6_prefix: BTreeMap::new(),
            })),
        }
    }

    fn purge_old_timestamps(&self, inner: &mut AddressFilterInner, cur_ts: Timestamp) {
        // v4
        {
            let mut dead_keys = Vec::<Ipv4Addr>::new();
            for (key, value) in &mut inner.conn_timestamps_by_ip4 {
                value.retain(|v| {
                    // keep timestamps that are less than a minute away
                    cur_ts.saturating_sub(*v) < TimestampDuration::new(60_000_000u64)
                });
                if value.is_empty() {
                    dead_keys.push(*key);
                }
            }
            for key in dead_keys {
                inner.conn_timestamps_by_ip4.remove(&key);
            }
        }
        // v6
        {
            let mut dead_keys = Vec::<Ipv6Addr>::new();
            for (key, value) in &mut inner.conn_timestamps_by_ip6_prefix {
                value.retain(|v| {
                    // keep timestamps that are less than a minute away
                    cur_ts.saturating_sub(*v) < TimestampDuration::new(60_000_000u64)
                });
                if value.is_empty() {
                    dead_keys.push(*key);
                }
            }
            for key in dead_keys {
                inner.conn_timestamps_by_ip6_prefix.remove(&key);
            }
        }
    }

    fn purge_old_punishments(&self, inner: &mut AddressFilterInner, cur_ts: Timestamp) {
        // v4
        {
            let mut dead_keys = Vec::<Ipv4Addr>::new();
            for (key, value) in &mut inner.punishments_by_ip4 {
                // Drop punishments older than the punishment duration
                if cur_ts.as_u64().saturating_sub(value.as_u64())
                    > self.unlocked_inner.punishment_duration_min as u64 * 60_000_000u64
                {
                    dead_keys.push(*key);
                }
            }
            for key in dead_keys {
                log_net!(debug ">>> FORGIVING: {}", key);
                inner.punishments_by_ip4.remove(&key);
            }
        }
        // v6
        {
            let mut dead_keys = Vec::<Ipv6Addr>::new();
            for (key, value) in &mut inner.punishments_by_ip6_prefix {
                // Drop punishments older than the punishment duration
                if cur_ts.as_u64().saturating_sub(value.as_u64())
                    > self.unlocked_inner.punishment_duration_min as u64 * 60_000_000u64
                {
                    dead_keys.push(*key);
                }
            }
            for key in dead_keys {
                log_net!(debug ">>> FORGIVING: {}", key);
                inner.punishments_by_ip6_prefix.remove(&key);
            }
        }
    }

    fn is_punished_inner(&self, inner: &AddressFilterInner, ipblock: IpAddr) -> bool {
        match ipblock {
            IpAddr::V4(v4) => {
                if inner.punishments_by_ip4.contains_key(&v4) {
                    return true;
                }
            }
            IpAddr::V6(v6) => {
                if inner.punishments_by_ip6_prefix.contains_key(&v6) {
                    return true;
                }
            }
        }
        false
    }

    pub fn is_punished(&self, addr: IpAddr) -> bool {
        let inner = self.inner.lock();
        let ipblock = ip_to_ipblock(
            self.unlocked_inner.max_connections_per_ip6_prefix_size,
            addr,
        );
        self.is_punished_inner(&*inner, ipblock)
    }

    pub fn punish(&self, addr: IpAddr) {
        log_net!(debug ">>> PUNISHED: {}", addr);
        let ts = get_aligned_timestamp();

        let ipblock = ip_to_ipblock(
            self.unlocked_inner.max_connections_per_ip6_prefix_size,
            addr,
        );

        let mut inner = self.inner.lock();
        match ipblock {
            IpAddr::V4(v4) => inner
                .punishments_by_ip4
                .entry(v4)
                .and_modify(|v| *v = ts)
                .or_insert(ts),
            IpAddr::V6(v6) => inner
                .punishments_by_ip6_prefix
                .entry(v6)
                .and_modify(|v| *v = ts)
                .or_insert(ts),
        };
    }

    pub async fn address_filter_task_routine(
        self,
        _stop_token: StopToken,
        _last_ts: Timestamp,
        cur_ts: Timestamp,
    ) -> EyreResult<()> {
        //
        let mut inner = self.inner.lock();
        self.purge_old_timestamps(&mut *inner, cur_ts);
        self.purge_old_punishments(&mut *inner, cur_ts);

        Ok(())
    }

    pub fn add_connection(&self, addr: IpAddr) -> Result<(), AddressFilterError> {
        let inner = &mut *self.inner.lock();

        let ipblock = ip_to_ipblock(
            self.unlocked_inner.max_connections_per_ip6_prefix_size,
            addr,
        );
        if self.is_punished_inner(inner, ipblock) {
            return Err(AddressFilterError::Punished);
        }

        let ts = get_aligned_timestamp();
        self.purge_old_timestamps(inner, ts);

        match ipblock {
            IpAddr::V4(v4) => {
                // See if we have too many connections from this ip block
                let cnt = inner.conn_count_by_ip4.entry(v4).or_default();
                assert!(*cnt <= self.unlocked_inner.max_connections_per_ip4);
                if *cnt == self.unlocked_inner.max_connections_per_ip4 {
                    warn!("address filter count exceeded: {:?}", v4);
                    return Err(AddressFilterError::CountExceeded);
                }
                // See if this ip block has connected too frequently
                let tstamps = inner.conn_timestamps_by_ip4.entry(v4).or_default();
                tstamps.retain(|v| {
                    // keep timestamps that are less than a minute away
                    ts.saturating_sub(*v) < TimestampDuration::new(60_000_000u64)
                });
                assert!(tstamps.len() <= self.unlocked_inner.max_connection_frequency_per_min);
                if tstamps.len() == self.unlocked_inner.max_connection_frequency_per_min {
                    warn!("address filter rate exceeded: {:?}", v4);
                    return Err(AddressFilterError::RateExceeded);
                }

                // If it's okay, add the counts and timestamps
                *cnt += 1;
                tstamps.push(ts);
            }
            IpAddr::V6(v6) => {
                // See if we have too many connections from this ip block
                let cnt = inner.conn_count_by_ip6_prefix.entry(v6).or_default();
                assert!(*cnt <= self.unlocked_inner.max_connections_per_ip6_prefix);
                if *cnt == self.unlocked_inner.max_connections_per_ip6_prefix {
                    warn!("address filter count exceeded: {:?}", v6);
                    return Err(AddressFilterError::CountExceeded);
                }
                // See if this ip block has connected too frequently
                let tstamps = inner.conn_timestamps_by_ip6_prefix.entry(v6).or_default();
                assert!(tstamps.len() <= self.unlocked_inner.max_connection_frequency_per_min);
                if tstamps.len() == self.unlocked_inner.max_connection_frequency_per_min {
                    warn!("address filter rate exceeded: {:?}", v6);
                    return Err(AddressFilterError::RateExceeded);
                }

                // If it's okay, add the counts and timestamps
                *cnt += 1;
                tstamps.push(ts);
            }
        }
        Ok(())
    }

    pub fn remove_connection(&mut self, addr: IpAddr) -> Result<(), AddressNotInTableError> {
        let mut inner = self.inner.lock();

        let ipblock = ip_to_ipblock(
            self.unlocked_inner.max_connections_per_ip6_prefix_size,
            addr,
        );

        let ts = get_aligned_timestamp();
        self.purge_old_timestamps(&mut *inner, ts);

        match ipblock {
            IpAddr::V4(v4) => {
                match inner.conn_count_by_ip4.entry(v4) {
                    Entry::Vacant(_) => {
                        return Err(AddressNotInTableError {});
                    }
                    Entry::Occupied(mut o) => {
                        let cnt = o.get_mut();
                        assert!(*cnt > 0);
                        if *cnt == 0 {
                            inner.conn_count_by_ip4.remove(&v4);
                        } else {
                            *cnt -= 1;
                        }
                    }
                };
            }
            IpAddr::V6(v6) => {
                match inner.conn_count_by_ip6_prefix.entry(v6) {
                    Entry::Vacant(_) => {
                        return Err(AddressNotInTableError {});
                    }
                    Entry::Occupied(mut o) => {
                        let cnt = o.get_mut();
                        assert!(*cnt > 0);
                        if *cnt == 0 {
                            inner.conn_count_by_ip6_prefix.remove(&v6);
                        } else {
                            *cnt -= 1;
                        }
                    }
                };
            }
        }
        Ok(())
    }
}