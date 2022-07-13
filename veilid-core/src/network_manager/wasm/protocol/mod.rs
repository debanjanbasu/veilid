pub mod wrtc;
pub mod ws;

use super::*;
use crate::xx::*;
use std::io;

#[derive(Debug)]
pub enum ProtocolNetworkConnection {
    #[allow(dead_code)]
    Dummy(DummyNetworkConnection),
    Ws(ws::WebsocketNetworkConnection),
    //WebRTC(wrtc::WebRTCNetworkConnection),
}

impl ProtocolNetworkConnection {
    pub async fn connect(
        local_address: Option<SocketAddr>,
        dial_info: &DialInfo,
    ) -> io::Result<ProtocolNetworkConnection> {
        match dial_info.protocol_type() {
            ProtocolType::UDP => {
                panic!("UDP dial info is not supported on WASM targets");
            }
            ProtocolType::TCP => {
                panic!("TCP dial info is not supported on WASM targets");
            }
            ProtocolType::WS | ProtocolType::WSS => {
                ws::WebsocketProtocolHandler::connect(local_address, dial_info).await
            }
        }
    }

    pub fn descriptor(&self) -> ConnectionDescriptor {
        match self {
            Self::Dummy(d) => d.descriptor(),
            Self::Ws(w) => w.descriptor(),
        }
    }

    // pub async fn close(&self) -> io::Result<()> {
    //     match self {
    //         Self::Dummy(d) => d.close(),
    //         Self::Ws(w) => w.close().await,
    //     }
    // }
    pub async fn send(&self, message: Vec<u8>) -> io::Result<()> {
        match self {
            Self::Dummy(d) => d.send(message),
            Self::Ws(w) => w.send(message).await,
        }
    }

    pub async fn recv(&self) -> io::Result<Vec<u8>> {
        match self {
            Self::Dummy(d) => d.recv(),
            Self::Ws(w) => w.recv().await,
        }
    }
}
