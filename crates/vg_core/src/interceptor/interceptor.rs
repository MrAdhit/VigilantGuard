use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;
use valence_protocol::bytes::BytesMut;
use valence_protocol::decoder::{decode_packet, PacketDecoder};
use valence_protocol::encoder::PacketEncoder;
use valence_protocol::Packet;

use crate::packet::PacketDirection;

pub enum InterceptResult {
    PASSTHROUGH,
    RETURN(Option<BytesMut>),
    IGNORE,
}

pub struct Interceptor<'b> {
    pub direction: PacketDirection,
    pub reader: Option<OwnedReadHalf>,
    pub writer: Option<OwnedWriteHalf>,
    pub encoder: PacketEncoder,
    pub decoder: PacketDecoder,
    pub frame: BytesMut,
    pub other: Option<&'b Mutex<Interceptor<'b>>>,
}

impl<'b> Interceptor<'b> {
    pub async fn gatekeeper<'a, P, F, Fut>(&'a mut self, intercept: F) -> anyhow::Result<P>
    where
        P: Packet<'a> + 'a,
        F: FnOnce(P, &'a OwnedReadHalf) -> Fut,
        Fut: futures::Future<Output = (InterceptResult, P)>,
    {
        loop {
            if let Some(frame) = self.decoder.try_next_packet()? {
                self.frame = frame;

                let packet: P = decode_packet(&self.frame)?;

                let result = intercept(packet, self.reader.as_ref().unwrap()).await;

                let packet = result.1;

                match result.0 {
                    InterceptResult::PASSTHROUGH => {
                        self.encoder.append_packet(&packet)?;

                        let bytes = self.encoder.take();

                        self.writer.as_mut().unwrap().write_all(&bytes).await?;
                    }
                    InterceptResult::RETURN(bytes) => {
                        if let Some(bytes) = bytes {
                            let bytes = bytes;

                            self.other.unwrap().lock().await.writer.as_mut().unwrap().write_all(&bytes).await?;
                        } else {
                            self.encoder.append_packet(&packet)?;

                            let bytes = self.encoder.take();

                            self.other.unwrap().lock().await.writer.as_mut().unwrap().write_all(&bytes).await?;
                        }
                    }
                    InterceptResult::IGNORE => {
                        return Ok(packet);
                    }
                }

                return Ok(packet);
            }

            self.decoder.reserve(4096);
            let mut buf = self.decoder.take_capacity();

            self.reader.as_mut().unwrap().read_buf(&mut buf).await?;

            self.decoder.queue_bytes(buf);
        }
    }
}
