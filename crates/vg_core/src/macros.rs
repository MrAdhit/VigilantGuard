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

#[macro_export]
macro_rules! colorizer {
    ($fmt_str:literal) => {{
        format!($fmt_str)
        .replace("c(reset)", "\x1b[0m")
        .replace("c(on_blue)", "\x1b[1;36;46m")
        .replace("c(dark_blue)", "\x1b[0;36m")
        .replace("c(on_red)", "\\x1b[1;31;41m")
        .replace("c(dark_red)", "\x1b[0;31m")
        .replace("c(bright_red)", "\x1b[1;31m")
    }};
    ($fmt_str:literal, $($args:expr),*) => {{
        format!($fmt_str, $($args),*)
        .replace("c(reset)", "\x1b[0m")
        .replace("c(on_blue)", "\x1b[1;36;46m")
        .replace("c(dark_blue)", "\x1b[0;36m")
        .replace("c(on_red)", "\x1b[1;31;41m")
        .replace("c(dark_red)", "\x1b[0;31m")
        .replace("c(bright_red)", "\x1b[1;31m")
    }}
}
