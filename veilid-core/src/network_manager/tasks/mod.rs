pub mod public_address_check;
pub mod rolling_transfers;

use super::*;

impl NetworkManager {
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
                                "NetworkManager rolling transfers task routine"
                            )),
                    )
                });
        }

        // Set public address check task
        {
            let this = self.clone();
            self.unlocked_inner
                .public_address_check_task
                .set_routine(move |s, l, t| {
                    Box::pin(
                        this.clone()
                            .public_address_check_task_routine(
                                s,
                                Timestamp::new(l),
                                Timestamp::new(t),
                            )
                            .instrument(trace_span!(
                                parent: None,
                                "public address check task routine"
                            )),
                    )
                });
        }
    }

    pub async fn tick(&self) -> EyreResult<()> {
        let routing_table = self.routing_table();
        let net = self.net();
        let receipt_manager = self.receipt_manager();

        // Run the rolling transfers task
        self.unlocked_inner.rolling_transfers_task.tick().await?;

        // Run the routing table tick
        routing_table.tick().await?;

        // Run the low level network tick
        net.tick().await?;

        // Run the receipt manager tick
        receipt_manager.tick().await?;

        // Purge the client whitelist
        self.purge_client_whitelist();

        Ok(())
    }

    pub(crate) async fn stop_tasks(&self) {
        debug!("stopping rolling transfers task");
        if let Err(e) = self.unlocked_inner.rolling_transfers_task.stop().await {
            warn!("rolling_transfers_task not stopped: {}", e);
        }
    }
}