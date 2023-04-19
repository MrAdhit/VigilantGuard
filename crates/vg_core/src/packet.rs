use valence_protocol::packet::c2s::handshake::handshake::NextState;
use valence_protocol::{Encode, Decode, Packet};
use valence_protocol::var_int::VarInt;

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet_id = 0x00]
pub struct QueryResponseS2cOwn {
    pub json: String,
}

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet_id = 0x00]
pub struct HandshakeC2sOwn {
    pub protocol_version: VarInt,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: NextState,
}

#[derive(Debug)]
pub enum PacketDirection {
    C2S,
    S2C
}