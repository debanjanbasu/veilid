use super::*;
use crate::xx::*;
use connection_table::*;
use network_connection::*;

///////////////////////////////////////////////////////////
// Connection manager

#[derive(Debug)]
struct ConnectionManagerInner {
    connection_table: ConnectionTable,
    stop_source: Option<StopSource>,
}

struct ConnectionManagerArc {
    network_manager: NetworkManager,
    inner: AsyncMutex<Option<ConnectionManagerInner>>,
}
impl core::fmt::Debug for ConnectionManagerArc {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ConnectionManagerArc")
            .field("inner", &self.inner)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionManager {
    arc: Arc<ConnectionManagerArc>,
}

impl ConnectionManager {
    fn new_inner(config: VeilidConfig) -> ConnectionManagerInner {
        ConnectionManagerInner {
            stop_source: Some(StopSource::new()),
            connection_table: ConnectionTable::new(config),
        }
    }
    fn new_arc(network_manager: NetworkManager) -> ConnectionManagerArc {
        ConnectionManagerArc {
            network_manager,
            inner: AsyncMutex::new(None),
        }
    }
    pub fn new(network_manager: NetworkManager) -> Self {
        Self {
            arc: Arc::new(Self::new_arc(network_manager)),
        }
    }

    pub fn network_manager(&self) -> NetworkManager {
        self.arc.network_manager.clone()
    }

    pub async fn startup(&self) {
        trace!("startup connection manager");
        let mut inner = self.arc.inner.lock().await;
        if inner.is_some() {
            panic!("shouldn't start connection manager twice without shutting it down first");
        }

        *inner = Some(Self::new_inner(self.network_manager().config()));
    }

    pub async fn shutdown(&self) {
        // Remove the inner from the lock
        let mut inner = {
            let mut inner_lock = self.arc.inner.lock().await;
            let inner = match inner_lock.take() {
                Some(v) => v,
                None => {
                    panic!("not started");
                }
            };
            inner
        };

        // Stop all the connections
        drop(inner.stop_source.take());

        // Wait for the connections to complete
        inner.connection_table.join().await;
    }

    // Returns a network connection if one already is established
    pub async fn get_connection(
        &self,
        descriptor: ConnectionDescriptor,
    ) -> Option<ConnectionHandle> {
        let mut inner = self.arc.inner.lock().await;
        let inner = match &mut *inner {
            Some(v) => v,
            None => {
                panic!("not started");
            }
        };
        inner.connection_table.get_connection(descriptor)
    }

    // Internal routine to register new connection atomically.
    // Registers connection in the connection table for later access
    // and spawns a message processing loop for the connection
    fn on_new_protocol_network_connection(
        &self,
        inner: &mut ConnectionManagerInner,
        conn: ProtocolNetworkConnection,
    ) -> Result<ConnectionHandle, String> {
        log_net!("on_new_protocol_network_connection: {:?}", conn);

        // Wrap with NetworkConnection object to start the connection processing loop
        let stop_token = match &inner.stop_source {
            Some(ss) => ss.token(),
            None => return Err("not creating connection because we are stopping".to_owned()),
        };

        let conn = NetworkConnection::from_protocol(self.clone(), stop_token, conn);
        let handle = conn.get_handle();
        // Add to the connection table
        inner.connection_table.add_connection(conn)?;
        Ok(handle)
    }

    // Called when we want to create a new connection or get the current one that already exists
    // This will kill off any connections that are in conflict with the new connection to be made
    // in order to make room for the new connection in the system's connection table
    pub async fn get_or_create_connection(
        &self,
        local_addr: Option<SocketAddr>,
        dial_info: DialInfo,
    ) -> Result<ConnectionHandle, String> {
        let mut inner = self.arc.inner.lock().await;
        let inner = match &mut *inner {
            Some(v) => v,
            None => {
                panic!("not started");
            }
        };

        log_net!(
            "== get_or_create_connection local_addr={:?} dial_info={:?}",
            local_addr.green(),
            dial_info.green()
        );

        let peer_address = dial_info.to_peer_address();
        let descriptor = match local_addr {
            Some(la) => {
                ConnectionDescriptor::new(peer_address, SocketAddress::from_socket_addr(la))
            }
            None => ConnectionDescriptor::new_no_local(peer_address),
        };

        // If any connection to this remote exists that has the same protocol, return it
        // Any connection will do, we don't have to match the local address

        if let Some(conn) = inner
            .connection_table
            .get_last_connection_by_remote(descriptor.remote())
        {
            log_net!(
                "== Returning existing connection local_addr={:?} peer_address={:?}",
                local_addr.green(),
                peer_address.green()
            );

            return Ok(conn);
        }

        // Drop any other protocols connections to this remote that have the same local addr
        // otherwise this connection won't succeed due to binding
        let mut killed = false;
        if let Some(local_addr) = local_addr {
            if local_addr.port() != 0 {
                for pt in [ProtocolType::TCP, ProtocolType::WS, ProtocolType::WSS] {
                    let pa = PeerAddress::new(descriptor.remote_address().clone(), pt);
                    for prior_descriptor in inner
                        .connection_table
                        .get_connection_descriptors_by_remote(pa)
                    {
                        let mut kill = false;
                        // See if the local address would collide
                        if let Some(prior_local) = prior_descriptor.local() {
                            if (local_addr.ip().is_unspecified()
                                || prior_local.to_ip_addr().is_unspecified()
                                || (local_addr.ip() == prior_local.to_ip_addr()))
                                && prior_local.port() == local_addr.port()
                            {
                                kill = true;
                            }
                        }
                        if kill {
                            log_net!(debug
                                ">< Terminating connection prior_descriptor={:?}",
                                prior_descriptor
                            );
                            if let Err(e) =
                                inner.connection_table.remove_connection(prior_descriptor)
                            {
                                log_net!(error e);
                            }
                            killed = true;
                        }
                    }
                }
            }
        }

        // Attempt new connection
        let mut retry_count = if killed { 2 } else { 0 };

        let conn = loop {
            match ProtocolNetworkConnection::connect(local_addr, dial_info.clone()).await {
                Ok(v) => break Ok(v),
                Err(e) => {
                    if retry_count == 0 {
                        break Err(e);
                    }
                    log_net!(debug "get_or_create_connection retries left: {}", retry_count);
                    retry_count -= 1;
                    intf::sleep(500).await;
                }
            }
        }?;

        self.on_new_protocol_network_connection(&mut *inner, conn)
    }

    ///////////////////////////////////////////////////////////////////////////////////////////////////////
    /// Callbacks

    // Called by low-level network when any connection-oriented protocol connection appears
    // either from incoming connections.
    pub(super) async fn on_accepted_protocol_network_connection(
        &self,
        conn: ProtocolNetworkConnection,
    ) -> Result<(), String> {
        let mut inner = self.arc.inner.lock().await;
        let inner = match &mut *inner {
            Some(v) => v,
            None => {
                // If we are shutting down, just drop this and return
                return Ok(());
            }
        };
        self.on_new_protocol_network_connection(inner, conn)
            .map(drop)
    }

    // Callback from network connection receive loop when it exits
    // cleans up the entry in the connection table
    pub(super) async fn report_connection_finished(&self, descriptor: ConnectionDescriptor) {
        let mut inner = self.arc.inner.lock().await;
        let inner = match &mut *inner {
            Some(v) => v,
            None => {
                // If we're shutting down, do nothing here
                return;
            }
        };

        if let Err(e) = inner.connection_table.remove_connection(descriptor) {
            log_net!(error e);
        }
    }
}
