use super::*;

impl RPCProcessor {
    // Sends a high level app message
    // Can be sent via all methods including relays and routes
    #[instrument(level = "trace", skip(self), ret, err)]
    pub async fn rpc_call_app_message(
        self,
        dest: Destination,
        message: Vec<u8>,
    ) -> Result<NetworkResult<()>, RPCError> {
        let app_message = RPCOperationAppMessage { message };
        let statement = RPCStatement::new(RPCStatementDetail::AppMessage(app_message));

        // Send the app message request
        self.statement(dest, statement).await
    }

    #[instrument(level = "trace", skip(self, msg), fields(msg.operation.op_id), ret, err)]
    pub(crate) async fn process_app_message(
        &self,
        msg: RPCMessage,
    ) -> Result<NetworkResult<()>, RPCError> {
        // Get the statement
        let app_message = match msg.operation.into_kind() {
            RPCOperationKind::Statement(s) => match s.into_detail() {
                RPCStatementDetail::AppMessage(s) => s,
                _ => panic!("not an app message"),
            },
            _ => panic!("not a statement"),
        };

        // Get the crypto kind used to send this question
        let crypto_kind = msg.header.crypto_kind();

        // Get the sender node id this came from
        let sender = msg
            .opt_sender_nr
            .as_ref()
            .map(|nr| nr.node_ids().get(crypto_kind).unwrap().key);

        // Pass the message up through the update callback
        let message = app_message.message;
        (self.unlocked_inner.update_callback)(VeilidUpdate::AppMessage(VeilidAppMessage {
            sender,
            message,
        }));

        Ok(NetworkResult::value(()))
    }
}
