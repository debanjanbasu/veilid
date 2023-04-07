use super::*;

///////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
pub enum Target {
    NodeId(PublicKey),     // Node by any of its public keys
    PrivateRoute(RouteId), // Remote private route by its id
}

pub struct RoutingContextInner {}

pub struct RoutingContextUnlockedInner {
    /// Safety routing requirements
    safety_selection: SafetySelection,
}

impl Drop for RoutingContextInner {
    fn drop(&mut self) {
        // self.api
        //     .borrow_mut()
        //     .routing_contexts
        //     //.remove(&self.id);
    }
}

#[derive(Clone)]
pub struct RoutingContext {
    /// Veilid API handle
    api: VeilidAPI,
    inner: Arc<Mutex<RoutingContextInner>>,
    unlocked_inner: Arc<RoutingContextUnlockedInner>,
}

impl RoutingContext {
    ////////////////////////////////////////////////////////////////

    pub(super) fn new(api: VeilidAPI) -> Self {
        Self {
            api,
            inner: Arc::new(Mutex::new(RoutingContextInner {})),
            unlocked_inner: Arc::new(RoutingContextUnlockedInner {
                safety_selection: SafetySelection::Unsafe(Sequencing::default()),
            }),
        }
    }

    pub fn with_privacy(self) -> Result<Self, VeilidAPIError> {
        self.with_custom_privacy(Stability::default())
    }

    pub fn with_custom_privacy(self, stability: Stability) -> Result<Self, VeilidAPIError> {
        let config = self.api.config()?;
        let c = config.get();

        Ok(Self {
            api: self.api.clone(),
            inner: Arc::new(Mutex::new(RoutingContextInner {})),
            unlocked_inner: Arc::new(RoutingContextUnlockedInner {
                safety_selection: SafetySelection::Safe(SafetySpec {
                    preferred_route: None,
                    hop_count: c.network.rpc.default_route_hop_count as usize,
                    stability,
                    sequencing: self.sequencing(),
                }),
            }),
        })
    }

    pub fn with_sequencing(self, sequencing: Sequencing) -> Self {
        Self {
            api: self.api.clone(),
            inner: Arc::new(Mutex::new(RoutingContextInner {})),
            unlocked_inner: Arc::new(RoutingContextUnlockedInner {
                safety_selection: match self.unlocked_inner.safety_selection {
                    SafetySelection::Unsafe(_) => SafetySelection::Unsafe(sequencing),
                    SafetySelection::Safe(safety_spec) => SafetySelection::Safe(SafetySpec {
                        preferred_route: safety_spec.preferred_route,
                        hop_count: safety_spec.hop_count,
                        stability: safety_spec.stability,
                        sequencing,
                    }),
                },
            }),
        }
    }

    fn sequencing(&self) -> Sequencing {
        match self.unlocked_inner.safety_selection {
            SafetySelection::Unsafe(sequencing) => sequencing,
            SafetySelection::Safe(safety_spec) => safety_spec.sequencing,
        }
    }

    pub fn api(&self) -> VeilidAPI {
        self.api.clone()
    }

    async fn get_destination(
        &self,
        target: Target,
    ) -> Result<rpc_processor::Destination, VeilidAPIError> {
        let rpc_processor = self.api.rpc_processor()?;

        match target {
            Target::NodeId(node_id) => {
                // Resolve node
                let mut nr = match rpc_processor.resolve_node(node_id).await {
                    Ok(Some(nr)) => nr,
                    Ok(None) => apibail_invalid_target!(),
                    Err(e) => return Err(e.into()),
                };
                // Apply sequencing to match safety selection
                nr.set_sequencing(self.sequencing());

                Ok(rpc_processor::Destination::Direct {
                    target: nr,
                    safety_selection: self.unlocked_inner.safety_selection,
                })
            }
            Target::PrivateRoute(rsid) => {
                // Get remote private route
                let rss = self.api.routing_table()?.route_spec_store();

                let Some(private_route) = rss.best_remote_private_route(&rsid) else {
                    apibail_invalid_target!();
                };

                Ok(rpc_processor::Destination::PrivateRoute {
                    private_route,
                    safety_selection: self.unlocked_inner.safety_selection,
                })
            }
        }
    }

    ////////////////////////////////////////////////////////////////
    // App-level Messaging

    #[instrument(level = "debug", err, skip(self))]
    pub async fn app_call(
        &self,
        target: Target,
        request: Vec<u8>,
    ) -> Result<Vec<u8>, VeilidAPIError> {
        let rpc_processor = self.api.rpc_processor()?;

        // Get destination
        let dest = self.get_destination(target).await?;

        // Send app message
        let answer = match rpc_processor.rpc_call_app_call(dest, request).await {
            Ok(NetworkResult::Value(v)) => v,
            Ok(NetworkResult::Timeout) => apibail_timeout!(),
            Ok(NetworkResult::ServiceUnavailable) => apibail_try_again!(),
            Ok(NetworkResult::NoConnection(e)) | Ok(NetworkResult::AlreadyExists(e)) => {
                apibail_no_connection!(e);
            }

            Ok(NetworkResult::InvalidMessage(message)) => {
                apibail_generic!(message);
            }
            Err(e) => return Err(e.into()),
        };

        Ok(answer.answer)
    }

    #[instrument(level = "debug", err, skip(self))]
    pub async fn app_message(
        &self,
        target: Target,
        message: Vec<u8>,
    ) -> Result<(), VeilidAPIError> {
        let rpc_processor = self.api.rpc_processor()?;

        // Get destination
        let dest = self.get_destination(target).await?;

        // Send app message
        match rpc_processor.rpc_call_app_message(dest, message).await {
            Ok(NetworkResult::Value(())) => {}
            Ok(NetworkResult::Timeout) => apibail_timeout!(),
            Ok(NetworkResult::ServiceUnavailable) => apibail_try_again!(),
            Ok(NetworkResult::NoConnection(e)) | Ok(NetworkResult::AlreadyExists(e)) => {
                apibail_no_connection!(e);
            }
            Ok(NetworkResult::InvalidMessage(message)) => {
                apibail_generic!(message);
            }
            Err(e) => return Err(e.into()),
        };

        Ok(())
    }

    ///////////////////////////////////
    /// DHT Records

    /// Creates a new DHT record a specified crypto kind and schema
    /// Returns the newly allocated DHT record's key if successful. The records is considered 'open' after the create operation succeeds.
    pub async fn create_dht_record(
        &self,
        kind: CryptoKind,
        schema: &DHTSchema,
    ) -> Result<TypedKey, VeilidAPIError> {
        let storage_manager = self.api.storage_manager()?;
        storage_manager
            .create_record(kind, schema, self.unlocked_inner.safety_selection)
            .await
    }

    /// Opens a DHT record at a specific key. Associates a secret if one is provided to provide writer capability.
    /// Returns the DHT record descriptor for the opened record if successful
    /// Records may only be opened or created . To re-open with a different routing context, first close the value.
    pub async fn open_dht_record(
        key: TypedKey,
        secret: Option<SecretKey>,
    ) -> Result<DHTRecordDescriptor, VeilidAPIError> {
        let storage_manager = self.api.storage_manager()?;
        storage_manager
            .open_record(key, secret, self.unlocked_inner.safety_selection)
            .await
    }

    /// Closes a DHT record at a specific key that was opened with create_dht_record or open_dht_record.
    /// Closing a record allows you to re-open it with a different routing context
    pub async fn close_dht_record(key: TypedKey) -> Result<(), VeilidAPIError> {
        let storage_manager = self.api.storage_manager()?;
        storage_manager.close_record(key).await
    }

    /// Deletes a DHT record at a specific key. If the record is opened, it must be closed before it is deleted.
    /// Deleting a record does not delete it from the network immediately, but will remove the storage of the record
    /// locally, and will prevent its value from being refreshed on the network by this node.
    pub async fn delete_dht_record(key: TypedKey) -> Result<(), VeilidAPIError> {
        let storage_manager = self.api.storage_manager()?;
        storage_manager.delete_record(key).await
    }

    /// Gets the latest value of a subkey
    /// May pull the latest value from the network, but by settings 'force_refresh' you can force a network data refresh
    /// Returns None if the value subkey has not yet been set
    /// Returns Some(data) if the value subkey has valid data
    pub async fn get_dht_value(
        &self,
        key: TypedKey,
        subkey: ValueSubkey,
        force_refresh: bool,
    ) -> Result<Option<ValueData>, VeilidAPIError> {
        let storage_manager = self.api.storage_manager()?;
        storage_manager.get_value(key, subkey, force_refresh).await
    }

    /// Pushes a changed subkey value to the network
    /// Returns None if the value was successfully put
    /// Returns Some(data) if the value put was older than the one available on the network
    pub async fn set_dht_value(
        &self,
        key: TypedKey,
        subkey: ValueSubkey,
        data: Vec<u8>,
    ) -> Result<Option<ValueData>, VeilidAPIError> {
        let storage_manager = self.api.storage_manager()?;
        storage_manager.set_value(key, subkey, data).await
    }

    /// Watches changes to an opened or created value
    /// Changes to subkeys within the subkey range are returned via a ValueChanged callback
    /// If the subkey range is empty, all subkey changes are considered
    /// Expiration can be infinite to keep the watch for the maximum amount of time
    /// Return value upon success is the amount of time allowed for the watch
    pub async fn watch_dht_values(
        &self,
        key: TypedKey,
        subkeys: &[ValueSubkeyRange],
        expiration: Timestamp,
        count: u32,
    ) -> Result<Timestamp, VeilidAPIError> {
        let storage_manager = self.api.storage_manager()?;
        storage_manager
            .watch_values(key, subkeys, expiration, count)
            .await
    }

    /// Cancels a watch early
    /// This is a convenience function that cancels watching all subkeys in a range
    pub async fn cancel_dht_watch(
        &self,
        key: TypedKey,
        subkeys: &[ValueSubkeyRange],
    ) -> Result<bool, VeilidAPIError> {
        let storage_manager = self.api.storage_manager()?;
        storage_manager.cancel_watch_value(key, subkey).await
    }

    ///////////////////////////////////
    /// Block Store

    pub async fn find_block(&self, _block_id: PublicKey) -> Result<Vec<u8>, VeilidAPIError> {
        panic!("unimplemented");
    }

    pub async fn supply_block(&self, _block_id: PublicKey) -> Result<bool, VeilidAPIError> {
        panic!("unimplemented");
    }
}
