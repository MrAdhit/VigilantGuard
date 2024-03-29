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

#[macro_export]
macro_rules! make_bytes {
    ($packet:expr) => {{
        let mut temp_enc = valence_protocol::encoder::PacketEncoder::new();
        temp_enc.append_packet(&$packet).unwrap();
        temp_enc.take()
    }};
}

// #[macro_export]
// macro_rules! gatekeeper {
//     ($direction_smol:ident; $type_camel:ident; $directionbig:ident; $type_snake:ident) => {
//         $direction_smol.lock().await.gatekeeper::<$type_camel, _, _>(|mut packet, reader| async move {
//             // let gate = crate::interceptor::gate::$direction_big::process(&mut packet, reader);
//             // gate.snake!($types);

//             (InterceptResult::PASSTHROUGH, packet)
//         }).await?
//     };
// }

// #[macro_export]
// macro_rules! make_gatekeeper {
//     ($direction:expr; $types:ident; $closure:expr) => {
//         $direction.lock().await.gatekeeper::<$types, _, _>($closure).await?
//     };

//     ($direction:ident; $types:ident) => {
//         // use vg_macro::{upper, snake};
//         // gatekeeper!($direction; $types; $direction; $types);
//         make_gatekeeper!($direction; QueryPongS2c; upper!($direction); snake!(QueryPongS2c));
//     };
//     ($d_l:ident; $t_c:ident; $d_u:expr; $t_s:expr) => {
//         $d_l.lock().await.gatekeeper::<$t_c, _, _>(|mut packet, reader| async move {
//             // let gate = crate::interceptor::gate::$direction_big::process(&mut packet, reader);
//             // gate.snake!($types);

//             (InterceptResult::PASSTHROUGH, packet)
//         }).await?
//     };
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
