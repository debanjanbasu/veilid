use crate::*;
use rpc_processor::*;

#[derive(Debug, Clone)]
pub struct RPCOperationSupplyBlockQ {
    pub block_id: DHTKey,
}

impl RPCOperationSupplyBlockQ {
    pub fn decode(
        reader: &veilid_capnp::operation_supply_block_q::Reader,
    ) -> Result<RPCOperationSupplyBlockQ, RPCError> {
        let bi_reader = reader.get_block_id().map_err(map_error_capnp_error!())?;
        let block_id = decode_block_id(&bi_reader);

        Ok(RPCOperationSupplyBlockQ { block_id })
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_supply_block_q::Builder,
    ) -> Result<(), RPCError> {
        let bi_builder = builder.init_block_id();
        encode_block_id(&self.block_id, &mut bi_builder)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum RPCOperationSupplyBlockA {
    Expiration(u64),
    Peers(Vec<PeerInfo>),
}

impl RPCOperationSupplyBlockA {
    pub fn decode(
        reader: &veilid_capnp::operation_supply_block_a::Reader,
    ) -> Result<RPCOperationSupplyBlockA, RPCError> {
        match reader.which().map_err(map_error_capnp_notinschema!())? {
            veilid_capnp::operation_supply_block_a::Which::Expiration(r) => {
                Ok(RPCOperationSupplyBlockA::Expiration(r))
            }
            veilid_capnp::operation_supply_block_a::Which::Peers(r) => {
                let peers_reader = r.map_err(map_error_capnp_error!())?;
                let mut peers = Vec::<PeerInfo>::with_capacity(
                    peers_reader
                        .len()
                        .try_into()
                        .map_err(map_error_internal!("too many peers"))?,
                );
                for p in peers_reader.iter() {
                    let peer_info = decode_peer_info(&p, true)?;
                    peers.push(peer_info);
                }

                Ok(RPCOperationSupplyBlockA::Peers(peers))
            }
        }
    }
    pub fn encode(
        &self,
        builder: &mut veilid_capnp::operation_supply_block_a::Builder,
    ) -> Result<(), RPCError> {
        match self {
            RPCOperationSupplyBlockA::Expiration(e) => {
                builder.set_expiration(*e);
            }
            RPCOperationSupplyBlockA::Peers(peers) => {
                let mut peers_builder = builder.init_peers(
                    peers
                        .len()
                        .try_into()
                        .map_err(map_error_internal!("invalid peers list length"))?,
                );
                for (i, peer) in peers.iter().enumerate() {
                    let mut pi_builder = peers_builder.reborrow().get(i as u32);
                    encode_peer_info(peer, &mut pi_builder)?;
                }
            }
        }

        Ok(())
    }
}