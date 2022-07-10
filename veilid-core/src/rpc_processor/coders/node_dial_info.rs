use crate::*;
use rpc_processor::*;

pub fn encode_node_dial_info(
    ndis: &NodeDialInfo,
    builder: &mut veilid_capnp::node_dial_info::Builder,
) -> Result<(), RPCError> {
    let mut ni_builder = builder.reborrow().init_node_id();
    encode_public_key(&ndis.node_id.key, &mut ni_builder)?;
    let mut di_builder = builder.reborrow().init_dial_info();
    encode_dial_info(&ndis.dial_info, &mut di_builder)?;
    Ok(())
}

pub fn decode_node_dial_info(
    reader: &veilid_capnp::node_dial_info::Reader,
) -> Result<NodeDialInfo, RPCError> {
    let node_id = decode_public_key(&reader.get_node_id().map_err(RPCError::map_protocol(
        "invalid public key in node_dial_info",
    ))?);
    let dial_info = decode_dial_info(&reader.get_dial_info().map_err(RPCError::map_protocol(
        "invalid dial_info in node_dial_info",
    ))?)?;

    Ok(NodeDialInfo {
        node_id: NodeId::new(node_id),
        dial_info,
    })
}
