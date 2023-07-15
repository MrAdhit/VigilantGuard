use valence_protocol::{Decode, Encode, Packet};

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet_id = 0x00]
pub struct QueryResponse {
    pub json: String,
}

#[derive(Clone, Debug, Encode, Decode, Packet)]
#[packet_id = 0x01]
pub struct QueryPong {
    pub payload: u64,
}
