use super::*;

impl RoutingTable {
    // Kick the queued buckets in the routing table to free dead nodes if necessary
    // Attempts to keep the size of the routing table down to the bucket depth
    #[instrument(level = "trace", skip(self), err)]
    pub(crate) async fn kick_buckets_task_routine(
        self,
        _stop_token: StopToken,
        _last_ts: Timestamp,
        cur_ts: Timestamp,
    ) -> EyreResult<()> {
        let kick_queue: Vec<usize> = core::mem::take(&mut *self.unlocked_inner.kick_queue.lock())
            .into_iter()
            .collect();
        let mut inner = self.inner.write();
        for idx in kick_queue {
            inner.kick_bucket(idx)
        }
        Ok(())
    }
}