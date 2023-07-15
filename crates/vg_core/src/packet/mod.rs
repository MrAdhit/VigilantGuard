pub mod c2s;
pub mod s2c;

#[derive(Debug)]
pub enum PacketDirection {
    C2S,
    S2C,
}
