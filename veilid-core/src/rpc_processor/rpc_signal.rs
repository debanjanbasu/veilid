use super::*;

impl RPCProcessor {
    // Sends a unidirectional signal to a node
    // Can be sent via relays but not routes. For routed 'signal' like capabilities, use AppMessage.
    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn rpc_call_signal(
        self,
        dest: Destination,
        signal_info: SignalInfo,
    ) -> Result<NetworkResult<()>, RPCError> {
        // Ensure destination never has a private route
        if matches!(
            dest,
            Destination::PrivateRoute {
                private_route: _,
                safety_selection: _
            }
        ) {
            return Err(RPCError::internal(
                "Never send signal requests over private routes",
            ));
        }

        let signal = RPCOperationSignal { signal_info };
        let statement = RPCStatement::new(RPCStatementDetail::Signal(signal));

        // Send the signal request
        network_result_try!(self.statement(dest, statement).await?);

        Ok(NetworkResult::value(()))
    }

    #[instrument(level = "trace", skip(self, msg), fields(msg.operation.op_id), err)]
    pub(crate) async fn process_signal(&self, msg: RPCMessage) -> Result<(), RPCError> {
        // Can't allow anything other than direct packets here, as handling reverse connections
        // or anything like via signals over private routes would deanonymize the route
        match &msg.header.detail {
            RPCMessageHeaderDetail::Direct(_) => {}
            RPCMessageHeaderDetail::SafetyRouted(_) | RPCMessageHeaderDetail::PrivateRouted(_) => {
                return Err(RPCError::protocol("signal must be direct"));
            }
        };

        // Get the statement
        let signal = match msg.operation.into_kind() {
            RPCOperationKind::Statement(s) => match s.into_detail() {
                RPCStatementDetail::Signal(s) => s,
                _ => panic!("not a signal"),
            },
            _ => panic!("not a statement"),
        };

        // Handle it
        let network_manager = self.network_manager();
        network_result_value_or_log!(debug network_manager
            .handle_signal(signal.signal_info)
            .await
            .map_err(RPCError::network)? => {
                return Ok(());
            }
        );

        Ok(())
    }
}
