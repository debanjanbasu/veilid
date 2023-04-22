mod address;
mod address_type_set;
mod dial_info;
mod dial_info_class;
mod dial_info_detail;
mod key256;
mod network_class;
mod node_info;
mod node_status;
mod nonce;
mod operations;
mod peer_info;
mod private_safety_route;
mod protocol_type_set;
mod sender_info;
mod sequencing;
mod signal_info;
mod signature512;
mod signed_direct_node_info;
mod signed_node_info;
mod signed_relayed_node_info;
mod signed_value_data;
mod signed_value_descriptor;
mod socket_address;
mod tunnel;
mod typed_key;
mod typed_signature;
mod value_detail; xxx eliminate value_detail

pub use address::*;
pub use address_type_set::*;
pub use dial_info::*;
pub use dial_info_class::*;
pub use dial_info_detail::*;
pub use key256::*;
pub use network_class::*;
pub use node_info::*;
pub use node_status::*;
pub use nonce::*;
pub use operations::*;
pub use peer_info::*;
pub use private_safety_route::*;
pub use protocol_type_set::*;
pub use sender_info::*;
pub use sequencing::*;
pub use signal_info::*;
pub use signature512::*;
pub use signed_direct_node_info::*;
pub use signed_node_info::*;
pub use signed_relayed_node_info::*;
pub use signed_value_data::*;
pub use signed_value_descriptor::*;
pub use socket_address::*;
pub use tunnel::*;
pub use typed_key::*;
pub use typed_signature::*;
pub use value_detail::*;

use super::*;

#[derive(Debug, Clone)]
pub struct DecodeContext {
    config: VeilidConfig,
}

#[derive(Debug, Clone)]
pub enum QuestionContext {
    GetValue(ValidateGetValueContext),
}

#[derive(Clone)]
pub struct RPCValidateContext {
    crypto: Crypto,
    question_context: Option<QuestionContext>,
}
