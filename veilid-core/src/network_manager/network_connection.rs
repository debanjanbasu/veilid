use super::*;
use futures_util::{FutureExt, StreamExt};
use stop_token::prelude::*;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        // No accept support for WASM
    } else {
        use async_std::net::*;

        ///////////////////////////////////////////////////////////
        // Accept

        pub trait ProtocolAcceptHandler: ProtocolAcceptHandlerClone + Send + Sync {
            fn on_accept(
                &self,
                stream: AsyncPeekStream,
                tcp_stream: TcpStream,
                peer_addr: SocketAddr,
            ) -> SystemPinBoxFuture<Result<Option<ProtocolNetworkConnection>, String>>;
        }

        pub trait ProtocolAcceptHandlerClone {
            fn clone_box(&self) -> Box<dyn ProtocolAcceptHandler>;
        }

        impl<T> ProtocolAcceptHandlerClone for T
        where
            T: 'static + ProtocolAcceptHandler + Clone,
        {
            fn clone_box(&self) -> Box<dyn ProtocolAcceptHandler> {
                Box::new(self.clone())
            }
        }
        impl Clone for Box<dyn ProtocolAcceptHandler> {
            fn clone(&self) -> Box<dyn ProtocolAcceptHandler> {
                self.clone_box()
            }
        }

        pub type NewProtocolAcceptHandler =
            dyn Fn(VeilidConfig, bool, SocketAddr) -> Box<dyn ProtocolAcceptHandler> + Send;
    }
}
///////////////////////////////////////////////////////////
// Dummy protocol network connection for testing

#[derive(Debug)]
pub struct DummyNetworkConnection {
    descriptor: ConnectionDescriptor,
}

impl DummyNetworkConnection {
    pub fn descriptor(&self) -> ConnectionDescriptor {
        self.descriptor.clone()
    }
    pub fn close(&self) -> Result<(), String> {
        Ok(())
    }
    pub fn send(&self, _message: Vec<u8>) -> Result<(), String> {
        Ok(())
    }
    pub fn recv(&self) -> Result<Vec<u8>, String> {
        Ok(Vec::new())
    }
}

///////////////////////////////////////////////////////////
// Top-level protocol independent network connection object

#[derive(Clone, Copy, Debug)]
enum RecvLoopAction {
    Send,
    Recv,
    Finish,
    Timeout,
}

#[derive(Debug, Clone)]
pub struct NetworkConnectionStats {
    last_message_sent_time: Option<u64>,
    last_message_recv_time: Option<u64>,
}

#[derive(Debug)]
pub struct NetworkConnection {
    descriptor: ConnectionDescriptor,
    processor: Option<MustJoinHandle<()>>,
    established_time: u64,
    stats: Arc<Mutex<NetworkConnectionStats>>,
    sender: flume::Sender<Vec<u8>>,
    stop_source: Option<StopSource>,
}

impl NetworkConnection {
    pub(super) fn dummy(descriptor: ConnectionDescriptor) -> Self {
        // Create handle for sending (dummy is immediately disconnected)
        let (sender, _receiver) = flume::bounded(intf::get_concurrency() as usize);

        Self {
            descriptor,
            processor: None,
            established_time: intf::get_timestamp(),
            stats: Arc::new(Mutex::new(NetworkConnectionStats {
                last_message_sent_time: None,
                last_message_recv_time: None,
            })),
            sender,
            stop_source: None,
        }
    }

    pub(super) fn from_protocol(
        connection_manager: ConnectionManager,
        manager_stop_token: StopToken,
        protocol_connection: ProtocolNetworkConnection,
    ) -> Self {
        // Get timeout
        let network_manager = connection_manager.network_manager();
        let inactivity_timeout = network_manager
            .config()
            .get()
            .network
            .connection_inactivity_timeout_ms;

        // Get descriptor
        let descriptor = protocol_connection.descriptor();

        // Create handle for sending
        let (sender, receiver) = flume::bounded(intf::get_concurrency() as usize);

        // Create stats
        let stats = Arc::new(Mutex::new(NetworkConnectionStats {
            last_message_sent_time: None,
            last_message_recv_time: None,
        }));

        let stop_source = StopSource::new();
        let local_stop_token = stop_source.token();

        // Spawn connection processor and pass in protocol connection
        let processor = MustJoinHandle::new(intf::spawn_local(Self::process_connection(
            connection_manager,
            local_stop_token,
            manager_stop_token,
            descriptor.clone(),
            receiver,
            protocol_connection,
            inactivity_timeout,
            stats.clone(),
        )));

        // Return the connection
        Self {
            descriptor,
            processor: Some(processor),
            established_time: intf::get_timestamp(),
            stats,
            sender,
            stop_source: Some(stop_source),
        }
    }

    pub fn connection_descriptor(&self) -> ConnectionDescriptor {
        self.descriptor.clone()
    }

    pub fn get_handle(&self) -> ConnectionHandle {
        ConnectionHandle::new(self.descriptor.clone(), self.sender.clone())
    }

    pub fn close(&mut self) {
        if let Some(stop_source) = self.stop_source.take() {
            // drop the stopper
            drop(stop_source);
        }
    }

    async fn send_internal(
        protocol_connection: &ProtocolNetworkConnection,
        stats: Arc<Mutex<NetworkConnectionStats>>,
        message: Vec<u8>,
    ) -> Result<(), String> {
        let ts = intf::get_timestamp();
        let out = protocol_connection.send(message).await;
        if out.is_ok() {
            let mut stats = stats.lock();
            stats.last_message_sent_time.max_assign(Some(ts));
        }
        out
    }
    async fn recv_internal(
        protocol_connection: &ProtocolNetworkConnection,
        stats: Arc<Mutex<NetworkConnectionStats>>,
    ) -> Result<Vec<u8>, String> {
        let ts = intf::get_timestamp();
        let out = protocol_connection.recv().await;
        if out.is_ok() {
            let mut stats = stats.lock();
            stats.last_message_recv_time.max_assign(Some(ts));
        }
        out
    }

    pub fn stats(&self) -> NetworkConnectionStats {
        let stats = self.stats.lock();
        stats.clone()
    }

    pub fn established_time(&self) -> u64 {
        self.established_time
    }

    // Connection receiver loop
    fn process_connection(
        connection_manager: ConnectionManager,
        local_stop_token: StopToken,
        manager_stop_token: StopToken,
        descriptor: ConnectionDescriptor,
        receiver: flume::Receiver<Vec<u8>>,
        protocol_connection: ProtocolNetworkConnection,
        connection_inactivity_timeout_ms: u32,
        stats: Arc<Mutex<NetworkConnectionStats>>,
    ) -> SystemPinBoxFuture<()> {
        Box::pin(async move {
            log_net!(
                "Starting process_connection loop for {:?}",
                descriptor.green()
            );

            let network_manager = connection_manager.network_manager();
            let mut unord = FuturesUnordered::new();
            let mut need_receiver = true;
            let mut need_sender = true;

            // Push mutable timer so we can reset it
            // Normally we would use an io::timeout here, but WASM won't support that, so we use a mutable sleep future
            let new_timer = || {
                intf::sleep(connection_inactivity_timeout_ms).then(|_| async {
                    // timeout
                    log_net!("connection timeout on {:?}", descriptor.green());
                    RecvLoopAction::Timeout
                })
            };
            let timer = MutableFuture::new(new_timer());

            unord.push(system_boxed(timer.clone()));

            loop {
                // Add another message sender future if necessary
                if need_sender {
                    need_sender = false;
                    let sender_fut = receiver.recv_async().then(|res| async {
                        match res {
                            Ok(message) => {
                                // send the packet
                                if let Err(e) = Self::send_internal(
                                    &protocol_connection,
                                    stats.clone(),
                                    message,
                                )
                                .await
                                {
                                    // Sending the packet along can fail, if so, this connection is dead
                                    log_net!(debug e);
                                    RecvLoopAction::Finish
                                } else {
                                    RecvLoopAction::Send
                                }
                            }
                            Err(e) => {
                                // All senders gone, shouldn't happen since we store one alongside the join handle
                                log_net!(warn e);
                                RecvLoopAction::Finish
                            }
                        }
                    });
                    unord.push(system_boxed(sender_fut));
                }

                // Add another message receiver future if necessary
                if need_receiver {
                    need_receiver = false;
                    let receiver_fut = Self::recv_internal(&protocol_connection, stats.clone())
                        .then(|res| async {
                            match res {
                                Ok(message) => {
                                    // Pass received messages up to the network manager for processing
                                    if let Err(e) = network_manager
                                        .on_recv_envelope(message.as_slice(), descriptor)
                                        .await
                                    {
                                        log_net!(error e);
                                        RecvLoopAction::Finish
                                    } else {
                                        RecvLoopAction::Recv
                                    }
                                }
                                Err(e) => {
                                    // Connection unable to receive, closed
                                    log_net!(warn e);
                                    RecvLoopAction::Finish
                                }
                            }
                        });

                    unord.push(system_boxed(receiver_fut));
                }

                // Process futures
                match unord
                    .next()
                    .timeout_at(local_stop_token.clone())
                    .timeout_at(manager_stop_token.clone())
                    .await
                    .and_then(std::convert::identity)   // flatten
                {
                    Ok(Some(RecvLoopAction::Send)) => {
                        // Don't reset inactivity timer if we're only sending
                        need_sender = true;
                    }
                    Ok(Some(RecvLoopAction::Recv)) => {
                        // Reset inactivity timer since we got something from this connection
                        timer.set(new_timer());

                        need_receiver = true;
                    }
                    Ok(Some(RecvLoopAction::Finish) | Some(RecvLoopAction::Timeout)) => {
                        break;
                    }
                    Ok(None) => {
                        // Should not happen
                        unreachable!();
                    }
                    Err(_) => {
                        // Either one of the stop tokens
                        break;
                    }
                }
            }

            log_net!(
                "== Connection loop finished descriptor={:?}",
                descriptor.green()
            );

            // Let the connection manager know the receive loop exited
            connection_manager
                .report_connection_finished(descriptor)
                .await;
        })
    }
}

// Resolves ready when the connection loop has terminated
impl Future for NetworkConnection {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        if let Some(mut processor) = self.processor.as_mut() {
            Pin::new(&mut processor).poll(cx)
        } else {
            task::Poll::Ready(())
        }
    }
}