use std::borrow::Cow;

use serde::{Serialize, Deserialize};
use valence_protocol::{Encode, Decode, var_int::VarInt, text::Text};

#[derive(Debug, Encode, Decode)]
pub struct C2sHandshakePacket {
    pub protocol: VarInt,
    pub addr: String,
    pub port: u16,
    pub next: PacketState,
}

#[derive(Debug, Encode, Decode)]
pub struct C2sQueryRequest {
    pub len: u8,
    pub payload: u8
}

#[derive(Debug, Encode, Decode)]
pub struct C2sPingRequest {
    pub payload: u64,
}

#[derive(Debug, Encode, Decode)]
pub struct S2cQueryResponse<'a> {
    pub json: &'a str,
}

#[derive(Debug, Encode, Decode)]
pub struct PacketHeader {
    pub len: VarInt,
    pub packet_id: VarInt,
}

#[derive(Debug, Encode, Decode)]
pub struct S2cDisconnect<'a> {
    pub reason: Cow<'a, Text>,
}

#[derive(Debug, Encode, Decode, Clone, Copy)]
pub enum PacketState {
    #[tag = 0]
    Handshake,
    #[tag = 1]
    Status,
    #[tag = 2]
    Login,
    #[tag = 2]
    Play,
}

impl TryFrom<i32> for PacketState {
    type Error = ();
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        dbg!(value);
        match value {
            0 => Ok(Self::Handshake),
            1 => Ok(Self::Status),
            2 => Ok(Self::Login),
            _ => Err(())
        }
    }
}

#[derive(Debug)]
pub enum PacketDirection {
    C2S,
    S2C
}

#[derive(Debug, Clone)]
pub enum PacketStage {
    C2sHandshake,
    C2sQueryRequest,
    S2cQueryResponse,
    C2sPingRequest,
    S2cPingResponse
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerListJson {
    pub version: ServerListJsonVersion,
    pub players: ServerListJsonPlayer,
    pub description: Text,
    pub favicon: Option<String>,
    pub enforces_secure_chat: Option<bool>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerListJsonVersion {
    name: String,
    protocol: u16,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerListJsonPlayer {
    max: usize,
    online: usize,
    sample: Option<Vec<ServerListJsonPlayerList>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerListJsonPlayerList {
    name: String,
    id: String,
}
