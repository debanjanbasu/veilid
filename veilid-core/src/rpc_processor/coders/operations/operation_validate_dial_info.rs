use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationValidateDialInfo {
    pub dial_info: DialInfo,
    pub receipt: Vec<u8>,
    pub redirect: bool,
}

impl RPCOperationValidateDialInfo {
    pub fn decode(
        reader: &veilid_capnp::operation_validate_dial_info::Reader,
    ) -> Result<RPCOperationValidateDialInfo, RPCError> {
        let di_reader = reader.get_dial_info().map_err(map_error_capnp_error!())?;
        let dial_info = decode_dial_info(&di_reader)?;
        let rcpt_reader = reader.get_receipt().map_err(map_error_capnp_error!())?;
        let receipt = rcpt_reader.to_vec();
        let redirect = reader.get_redirect();

        Ok(RPCOperationValidateDialInfo {
            dial_info,
            receipt,
            redirect,
        })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_validate_dial_info::Builder,
    ) -> Result<(), RPCError> {
        let di_builder = builder.init_dial_info();
        encode_dial_info(&self.dial_info, &mut di_builder)?;
        builder.set_receipt(&self.receipt);
        builder.set_redirect(self.redirect);
        Ok(())
    }
}