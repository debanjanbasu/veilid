pub mod bootstrap;
pub mod kick_buckets;
pub mod peer_minimum_refresh;
pub mod ping_validator;
pub mod private_route_management;
pub mod relay_management;
pub mod rolling_transfers;

use super::*;

impl RoutingTable {
    pub(crate) fn start_tasks(&self) {
        // Set rolling transfers tick task
        {
            let this = self.clone();
            self.unlocked_inner
                .rolling_transfers_task
                .set_routine(move |s, l, t| {
                    Box::pin(
                        this.clone()
                            .rolling_transfers_task_routine(s, Timestamp::new(l), Timestamp::new(t))
                            .instrument(trace_span!(
                                parent: None,
                                "RoutingTable rolling transfers task routine"
                            )),
                    )
                });
        }

        // Set kick buckets tick task
        {
            let this = self.clone();
            self.unlocked_inner
                .kick_buckets_task
                .set_routine(move |s, l, t| {
                    Box::pin(
                        this.clone()
                            .kick_buckets_task_routine(s, Timestamp::new(l), Timestamp::new(t))
                            .instrument(trace_span!(parent: None, "kick buckets task routine")),
                    )
                });
        }

        // Set bootstrap tick task
        {
            let this = self.clone();
            self.unlocked_inner
                .bootstrap_task
                .set_routine(move |s, _l, _t| {
                    Box::pin(
                        this.clone()
                            .bootstrap_task_routine(s)
                            .instrument(trace_span!(parent: None, "bootstrap task routine")),
                    )
                });
        }

        // Set peer minimum refresh tick task
        {
            let this = self.clone();
            self.unlocked_inner
                .peer_minimum_refresh_task
                .set_routine(move |s, _l, _t| {
                    Box::pin(
                        this.clone()
                            .peer_minimum_refresh_task_routine(s)
                            .instrument(trace_span!(
                                parent: None,
                                "peer minimum refresh task routine"
                            )),
                    )
                });
        }

        // Set ping validator tick task
        {
            let this = self.clone();
            self.unlocked_inner
                .ping_validator_task
                .set_routine(move |s, l, t| {
                    Box::pin(
                        this.clone()
                            .ping_validator_task_routine(s, Timestamp::new(l), Timestamp::new(t))
                            .instrument(trace_span!(parent: None, "ping validator task routine")),
                    )
                });
        }

        // Set relay management tick task
        {
            let this = self.clone();
            self.unlocked_inner
                .relay_management_task
                .set_routine(move |s, l, t| {
                    Box::pin(
                        this.clone()
                            .relay_management_task_routine(s, Timestamp::new(l), Timestamp::new(t))
                            .instrument(trace_span!(parent: None, "relay management task routine")),
                    )
                });
        }

        // Set private route management tick task
        {
            let this = self.clone();
            self.unlocked_inner
                .private_route_management_task
                .set_routine(move |s, l, t| {
                    Box::pin(
                        this.clone()
                            .private_route_management_task_routine(
                                s,
                                Timestamp::new(l),
                                Timestamp::new(t),
                            )
                            .instrument(trace_span!(
                                parent: None,
                                "private route management task routine"
                            )),
                    )
                });
        }
    }

    /// Ticks about once per second
    /// to run tick tasks which may run at slower tick rates as configured
    pub async fn tick(&self) -> EyreResult<()> {
        // Do rolling transfers every ROLLING_TRANSFERS_INTERVAL_SECS secs
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // Kick buckets task
        let kick_bucket_queue_count = self.unlocked_inner.kick_queue.lock().len();
        if kick_bucket_queue_count > 0 {
            self.unlocked_inner.kick_buckets_task.tick().await?;
        }

        // See how many live PublicInternet entries we have
        let live_public_internet_entry_count = self.get_entry_count(
            RoutingDomain::PublicInternet.into(),
            BucketEntryState::Unreliable,
        );
        let min_peer_count = self.with_config(|c| c.network.dht.min_peer_count as usize);

        // If none, then add the bootstrap nodes to it
        if live_public_internet_entry_count == 0 {
            self.unlocked_inner.bootstrap_task.tick().await?;
        }
        // If we still don't have enough peers, find nodes until we do
        else if !self.unlocked_inner.bootstrap_task.is_running()
            && live_public_internet_entry_count < min_peer_count
        {
            self.unlocked_inner.peer_minimum_refresh_task.tick().await?;
        }

        // Ping validate some nodes to groom the table
        self.unlocked_inner.ping_validator_task.tick().await?;

        // Run the relay management task
        self.unlocked_inner.relay_management_task.tick().await?;

        // Run the private route management task
        self.unlocked_inner
            .private_route_management_task
            .tick()
            .await?;

        Ok(())
    }

    pub(crate) async fn stop_tasks(&self) {
        // Cancel all tasks being ticked
        debug!("stopping rolling transfers task");
        if let Err(e) = self.unlocked_inner.rolling_transfers_task.stop().await {
            error!("rolling_transfers_task not stopped: {}", e);
        }
        debug!("stopping kick buckets task");
        if let Err(e) = self.unlocked_inner.kick_buckets_task.stop().await {
            error!("kick_buckets_task not stopped: {}", e);
        }
        debug!("stopping bootstrap task");
        if let Err(e) = self.unlocked_inner.bootstrap_task.stop().await {
            error!("bootstrap_task not stopped: {}", e);
        }
        debug!("stopping peer minimum refresh task");
        if let Err(e) = self.unlocked_inner.peer_minimum_refresh_task.stop().await {
            error!("peer_minimum_refresh_task not stopped: {}", e);
        }
        debug!("stopping ping_validator task");
        if let Err(e) = self.unlocked_inner.ping_validator_task.stop().await {
            error!("ping_validator_task not stopped: {}", e);
        }
        debug!("stopping relay management task");
        if let Err(e) = self.unlocked_inner.relay_management_task.stop().await {
            warn!("relay_management_task not stopped: {}", e);
        }
        debug!("stopping private route management task");
        if let Err(e) = self
            .unlocked_inner
            .private_route_management_task
            .stop()
            .await
        {
            warn!("private_route_management_task not stopped: {}", e);
        }
    }
}