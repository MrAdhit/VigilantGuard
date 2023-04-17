use vg_macro::create_colorizer;

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

// #[macro_export]
// macro_rules! colorizer {
//     ($fmt_str:literal) => {{
//         format!($fmt_str)
//         .replace("c(reset)", "\x1b[0m")
//         .replace("c(on_blue)", "\x1b[1;36;46m")
//         .replace("c(dark_blue)", "\x1b[0;36m")
//         .replace("c(on_red)", "\\x1b[1;31;41m")
//         .replace("c(dark_red)", "\x1b[0;31m")
//         .replace("c(bright_red)", "\x1b[1;31m")
//     }};
//     ($fmt_str:literal, $($args:expr),*) => {{
//         format!($fmt_str, $($args),*)
//         .replace("c(reset)", "\x1b[0m")
//         .replace("c(on_blue)", "\x1b[1;36;46m")
//         .replace("c(dark_blue)", "\x1b[0;36m")
//         .replace("c(on_red)", "\x1b[1;31;41m")
//         .replace("c(dark_red)", "\x1b[0;31m")
//         .replace("c(bright_red)", "\x1b[1;31m")
//     }}
// }

create_colorizer! {
    "reset" = "\x1b[0m"
    "gray" = "\x1b[1;30m"
    "black" = "\x1b[0;30m"
    "on_black" = "\x1b[1;30;40m"
    "bright_red" = "\x1b[1;31m"
    "dark_red" = "\x1b[0;31m"
    "on_red" = "\x1b[1;31;41m"
    "bright_green" = "\x1b[1;32m"
    "dark_green" = "\x1b[0;32m"
    "on_green" = "\x1b[1;32;42m"
    "bright_yellow" = "\x1b[1;33m"
    "dark_yellow" = "\x1b[0;33m"
    "on_yellow" = "\x1b[1;33;43m"
    "bright_blue" = "\x1b[1;34m"
    "dark_blue" = "\x1b[0;34m"
    "on_blue" = "\x1b[1;34;44m"
    "bright_purple" = "\x1b[1;35m"
    "dark_purple" = "\x1b[0;35m"
    "on_purple" = "\x1b[1;35;45m"
    "bright_cyan" = "\x1b[1;36m"
    "dark_cyan" = "\x1b[0;36m"
    "on_cyan" = "\x1b[1;36;46m"
    "bright_white" = "\x1b[1;37m"
    "dark_white" = "\x1b[0;37m"
    "on_white" = "\x1b[1;37;47m"
}
