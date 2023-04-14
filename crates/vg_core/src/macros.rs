#[macro_export]
macro_rules! printhex {
    ($v:expr) => {
        println!("[{}:{}] {} = {:02X?}", file!(), line!(), stringify!($v), $v);
    };
}

#[macro_export]
macro_rules! encode_packet {
    ($packet:ident, $writer:expr) => {
        let mut w = Vec::new();

        $packet.encode(&mut w).unwrap();
        $writer.append(&mut w);
    };
    ($packet_id:expr, $packet:expr, $writer:expr) => {
        let mut w = Vec::new();

        w.push($packet_id);
        $packet.encode(&mut w).unwrap();
        let mut len = Vec::new();
        valence_protocol::var_int::VarInt::encode(&valence_protocol::var_int::VarInt(w.len() as i32), &mut len).unwrap();
        $writer.append(&mut len);
        $writer.append(&mut w);
    };
}
