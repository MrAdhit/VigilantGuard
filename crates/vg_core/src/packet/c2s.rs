use valence_protocol::{packet::c2s::handshake::handshake::NextState, uuid::Uuid, var_int::VarInt, Decode, Encode, Packet};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet_id = 0x00]
pub struct Handshake {
    pub protocol_version: VarInt,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: NextState,
}

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet_id = 0x00]
pub struct QueryRequest;

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet_id = 0x01]
pub struct QueryPing {
    pub payload: u64,
}

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet_id = 0x00]
pub struct LoginHello {
    pub username: String,
    pub profile_id: Option<Uuid>,
}
