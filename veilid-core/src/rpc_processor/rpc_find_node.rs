use super::*;

impl RPCProcessor {
    /// Send FindNodeQ RPC request, receive FindNodeA answer
    /// Can be sent via all methods including relays
    /// Safety routes may be used, but never private routes.
    /// Because this leaks information about the identity of the node itself,
    /// replying to this request received over a private route will leak
    /// the identity of the node and defeat the private route.
    #[instrument(level = "trace", skip(self), err)]
    pub async fn rpc_call_find_node(
        self,
        dest: Destination,
        node_id: TypedKey,
    ) -> Result<NetworkResult<Answer<Vec<PeerInfo>>>, RPCError> {
        // Ensure destination never has a private route
        if matches!(
            dest,
            Destination::PrivateRoute {
                private_route: _,
                safety_selection: _
            }
        ) {
            return Err(RPCError::internal(
                "Never send find node requests over private routes",
            ));
        }

        let find_node_q_detail = RPCQuestionDetail::FindNodeQ(RPCOperationFindNodeQ::new(node_id));
        let find_node_q = RPCQuestion::new(
            network_result_try!(self.get_destination_respond_to(&dest)?),
            find_node_q_detail,
        );

        let debug_string = format!("FindNode(node_id={}) => {}", node_id, dest);

        // Send the find_node request
        let waitable_reply = network_result_try!(self.question(dest, find_node_q, None).await?);

        // Wait for reply
        let (msg, latency) = match self.wait_for_reply(waitable_reply, debug_string).await? {
            TimeoutOr::Timeout => return Ok(NetworkResult::Timeout),
            TimeoutOr::Value(v) => v,
        };

        // Get the right answer type
        let (_, _, _, kind) = msg.operation.destructure();
        let find_node_a = match kind {
            RPCOperationKind::Answer(a) => match a.destructure() {
                RPCAnswerDetail::FindNodeA(a) => a,
                _ => return Ok(NetworkResult::invalid_message("not a find_node answer")),
            },
            _ => return Ok(NetworkResult::invalid_message("not an answer")),
        };

        // Verify peers are in the correct peer scope
        let peers = find_node_a.destructure();

        for peer_info in &peers {
            if !self.verify_node_info(RoutingDomain::PublicInternet, peer_info.signed_node_info()) {
                return Ok(NetworkResult::invalid_message(
                    "find_node response has invalid peer scope",
                ));
            }
        }

        Ok(NetworkResult::value(Answer::new(latency, peers)))
    }

    #[instrument(level = "trace", skip(self, msg), fields(msg.operation.op_id), ret, err)]
    pub(crate) async fn process_find_node_q(
        &self,
        msg: RPCMessage,
    ) -> Result<NetworkResult<()>, RPCError> {
        // Ensure this never came over a private route, safety route is okay though
        match &msg.header.detail {
            RPCMessageHeaderDetail::Direct(_) | RPCMessageHeaderDetail::SafetyRouted(_) => {}
            RPCMessageHeaderDetail::PrivateRouted(_) => {
                return Ok(NetworkResult::invalid_message(
                    "not processing find node request over private route",
                ))
            }
        }

        // Get the question
        let kind = msg.operation.kind().clone();
        let find_node_q = match kind {
            RPCOperationKind::Question(q) => match q.destructure() {
                (_, RPCQuestionDetail::FindNodeQ(q)) => q,
                _ => panic!("not a findnode question"),
            },
            _ => panic!("not a question"),
        };
        let node_id = find_node_q.destructure();

        // Get a chunk of the routing table near the requested node id
        let routing_table = self.routing_table();
        let closest_nodes = network_result_try!(routing_table.find_all_closest_peers(node_id));

        // Make FindNode answer
        let find_node_a = RPCOperationFindNodeA::new(closest_nodes)?;

        // Send FindNode answer
        self.answer(msg, RPCAnswer::new(RPCAnswerDetail::FindNodeA(find_node_a)))
            .await
    }
}
