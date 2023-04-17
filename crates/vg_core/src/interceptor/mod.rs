use std::borrow::Cow;

use valence_protocol::{text::Text, Encode, var_int::VarInt};

use crate::packet::{S2cDisconnect, ToBuffer};

pub mod front;
pub mod middle;

fn disconnect_with_reason(reason: Text) -> Vec<u8> {
    let mut writer = Vec::new();
    reason.encode(&mut writer).unwrap();
    let len = writer.len();
    let mut message = S2cDisconnect { len: VarInt(len as i32), packet_id: VarInt(0x00), reason: Cow::Owned(reason) };
    message.to_buffer()
}