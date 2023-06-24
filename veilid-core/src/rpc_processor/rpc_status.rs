use super::*;

#[derive(Clone, Debug, PartialEq, PartialOrd, Ord, Eq, Hash, Default)]
pub struct SenderInfo {
    pub socket_address: SocketAddress,
}

impl RPCProcessor {
    // Send StatusQ RPC request, receive StatusA answer
    // Can be sent via relays or routes, but will have less information via routes
    // sender:
    // unsafe -> node status
    // safe -> nothing
    // receiver:
    // direct -> node status + sender info
    // safety -> node status
    // private -> nothing
    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn rpc_call_status(
        self,
        dest: Destination,
    ) -> Result<NetworkResult<Answer<Option<SenderInfo>>>, RPCError> {
        let (opt_target_nr, routing_domain, node_status) = match dest.get_safety_selection() {
            SafetySelection::Unsafe(_) => {
                let (opt_target_nr, routing_domain) = match &dest {
                    Destination::Direct {
                        target,
                        safety_selection: _,
                    } => {
                        let routing_domain = match target.best_routing_domain() {
                            Some(rd) => rd,
                            None => {
                                // Because this exits before calling 'question()',
                                // a failure to find a routing domain constitutes a send failure
                                let send_ts = get_aligned_timestamp();
                                self.record_send_failure(
                                    RPCKind::Question,
                                    send_ts,
                                    target.clone(),
                                    None,
                                    None,
                                );
                                return Ok(NetworkResult::no_connection_other(
                                    "no routing domain for target",
                                ));
                            }
                        };
                        (Some(target.clone()), routing_domain)
                    }
                    Destination::Relay {
                        relay,
                        target,
                        safety_selection: _,
                    } => {
                        let routing_domain = match relay.best_routing_domain() {
                            Some(rd) => rd,
                            None => {
                                // Because this exits before calling 'question()',
                                // a failure to find a routing domain constitutes a send failure for both the target and its relay
                                let send_ts = get_aligned_timestamp();
                                self.record_send_failure(
                                    RPCKind::Question,
                                    send_ts,
                                    relay.clone(),
                                    None,
                                    None,
                                );
                                self.record_send_failure(
                                    RPCKind::Question,
                                    send_ts,
                                    target.clone(),
                                    None,
                                    None,
                                );
                                return Ok(NetworkResult::no_connection_other(
                                    "no routing domain for peer",
                                ));
                            }
                        };
                        (Some(target.clone()), routing_domain)
                    }
                    Destination::PrivateRoute {
                        private_route: _,
                        safety_selection: _,
                    } => (None, RoutingDomain::PublicInternet),
                };

                let node_status = Some(self.network_manager().generate_node_status(routing_domain));
                (opt_target_nr, routing_domain, node_status)
            }
            SafetySelection::Safe(_) => {
                let routing_domain = RoutingDomain::PublicInternet;
                let node_status = None;
                (None, routing_domain, node_status)
            }
        };

        let status_q = RPCOperationStatusQ::new(node_status);
        let question = RPCQuestion::new(
            network_result_try!(self.get_destination_respond_to(&dest)?),
            RPCQuestionDetail::StatusQ(status_q),
        );

        let debug_string = format!("Status => {}", dest);

        // Send the info request
        let waitable_reply =
            network_result_try!(self.question(dest.clone(), question, None).await?);

        // Note what kind of ping this was and to what peer scope
        let send_data_kind = waitable_reply.send_data_kind;

        // Wait for reply
        let (msg, latency) = match self.wait_for_reply(waitable_reply, debug_string).await? {
            TimeoutOr::Timeout => return Ok(NetworkResult::Timeout),
            TimeoutOr::Value(v) => v,
        };

        // Get the right answer type
        let (_, _, _, kind) = msg.operation.destructure();
        let status_a = match kind {
            RPCOperationKind::Answer(a) => match a.destructure() {
                RPCAnswerDetail::StatusA(a) => a,
                _ => return Ok(NetworkResult::invalid_message("not a status answer")),
            },
            _ => return Ok(NetworkResult::invalid_message("not an answer")),
        };
        let (a_node_status, sender_info) = status_a.destructure();

        // Ensure the returned node status is the kind for the routing domain we asked for
        if let Some(target_nr) = opt_target_nr {
            if let Some(a_node_status) = a_node_status {
                match routing_domain {
                    RoutingDomain::PublicInternet => {
                        if !matches!(a_node_status, NodeStatus::PublicInternet(_)) {
                            return Ok(NetworkResult::invalid_message(
                                "node status doesn't match PublicInternet routing domain",
                            ));
                        }
                    }
                    RoutingDomain::LocalNetwork => {
                        if !matches!(a_node_status, NodeStatus::LocalNetwork(_)) {
                            return Ok(NetworkResult::invalid_message(
                                "node status doesn't match LocalNetwork routing domain",
                            ));
                        }
                    }
                }

                // Update latest node status in routing table
                target_nr.update_node_status(a_node_status.clone());
            }
        }

        // Report sender_info IP addresses to network manager
        // Don't need to validate these addresses for the current routing domain
        // the address itself is irrelevant, and the remote node can lie anyway
        let mut opt_sender_info = None;
        match dest {
            Destination::Direct {
                target,
                safety_selection,
            } => {
                if matches!(safety_selection, SafetySelection::Unsafe(_)) {
                    if let Some(sender_info) = sender_info {
                        match send_data_kind {
                            SendDataKind::Direct(connection_descriptor) => {
                                // Directly requested status that actually gets sent directly and not over a relay will tell us what our IP address appears as
                                // If this changes, we'd want to know about that to reset the networking stack
                                match routing_domain {
                                    RoutingDomain::PublicInternet => self
                                        .network_manager()
                                        .report_public_internet_socket_address(
                                            sender_info.socket_address,
                                            connection_descriptor,
                                            target,
                                        ),
                                    RoutingDomain::LocalNetwork => {
                                        self.network_manager().report_local_network_socket_address(
                                            sender_info.socket_address,
                                            connection_descriptor,
                                            target,
                                        )
                                    }
                                }
                            }
                            SendDataKind::Indirect => {
                                // Do nothing in this case, as the socket address returned here would be for any node other than ours
                            }
                            SendDataKind::Existing(_) => {
                                // Do nothing in this case, as an existing connection could not have a different public address or it would have been reset
                            }
                        };
                        opt_sender_info = Some(sender_info.clone());
                    }
                }
            }
            Destination::Relay {
                relay: _,
                target: _,
                safety_selection: _,
            }
            | Destination::PrivateRoute {
                private_route: _,
                safety_selection: _,
            } => {
                // sender info is irrelevant over relays and routes
            }
        };
        Ok(NetworkResult::value(Answer::new(latency, opt_sender_info)))
    }

    #[instrument(level = "trace", skip(self, msg), fields(msg.operation.op_id), ret, err)]
    pub(crate) async fn process_status_q(
        &self,
        msg: RPCMessage,
    ) -> Result<NetworkResult<()>, RPCError> {
        // Get the question
        let kind = msg.operation.kind().clone();
        let status_q = match kind {
            RPCOperationKind::Question(q) => match q.destructure() {
                (_, RPCQuestionDetail::StatusQ(q)) => q,
                _ => panic!("not a status question"),
            },
            _ => panic!("not a question"),
        };
        let q_node_status = status_q.destructure();

        let (node_status, sender_info) = match &msg.header.detail {
            RPCMessageHeaderDetail::Direct(detail) => {
                let connection_descriptor = detail.connection_descriptor;
                let routing_domain = detail.routing_domain;

                // Ensure the node status from the question is the kind for the routing domain we received the request in
                if let Some(q_node_status) = q_node_status {
                    match routing_domain {
                        RoutingDomain::PublicInternet => {
                            if !matches!(q_node_status, NodeStatus::PublicInternet(_)) {
                                return Ok(NetworkResult::invalid_message(
                                    "node status doesn't match PublicInternet routing domain",
                                ));
                            }
                        }
                        RoutingDomain::LocalNetwork => {
                            if !matches!(q_node_status, NodeStatus::LocalNetwork(_)) {
                                return Ok(NetworkResult::invalid_message(
                                    "node status doesn't match LocalNetwork routing domain",
                                ));
                            }
                        }
                    }

                    // update node status for the requesting node to our routing table
                    if let Some(sender_nr) = msg.opt_sender_nr.clone() {
                        // Update latest node status in routing table for the statusq sender
                        sender_nr.update_node_status(q_node_status.clone());
                    }
                }

                // Get the peer address in the returned sender info
                let sender_info = SenderInfo {
                    socket_address: *connection_descriptor.remote_address(),
                };

                // Make status answer
                let node_status = self.network_manager().generate_node_status(routing_domain);
                (Some(node_status), Some(sender_info))
            }
            RPCMessageHeaderDetail::SafetyRouted(_) => {
                // Make status answer
                let node_status = self
                    .network_manager()
                    .generate_node_status(RoutingDomain::PublicInternet);
                (Some(node_status), None)
            }
            RPCMessageHeaderDetail::PrivateRouted(_) => (None, None),
        };

        // Make status answer
        let status_a = RPCOperationStatusA::new(node_status, sender_info);

        // Send status answer
        self.answer(msg, RPCAnswer::new(RPCAnswerDetail::StatusA(status_a)))
            .await
    }
}
