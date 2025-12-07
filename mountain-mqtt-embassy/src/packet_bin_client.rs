use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::{Receiver, Sender},
};
use embassy_time::{Duration, WithTimeout};
use mountain_mqtt::{
    client::ClientError,
    codec::{
        mqtt_writer::{MqttBufWriter, MqttWriter},
        write,
    },
    error::PacketWriteError,
    packets::packet::Packet,
};

use crate::packet_bin::PacketBin;

pub struct PacketBinClient<'a, M, const N: usize>
where
    M: RawMutex,
{
    sender: Sender<'a, M, PacketBin<N>, 1>,
    receiver: Receiver<'a, M, PacketBin<N>, 1>,
}

impl<'a, M, const N: usize> PacketBinClient<'a, M, N>
where
    M: RawMutex,
{
    pub fn new(
        sender: Sender<'a, M, PacketBin<N>, 1>,
        receiver: Receiver<'a, M, PacketBin<N>, 1>,
    ) -> Self {
        Self { sender, receiver }
    }

    /// Send a packet
    /// Cancel-safe: This just sends to a [`Sender`]
    pub async fn send(&mut self, message: PacketBin<N>) {
        self.sender.send(message).await
    }

    /// Receive a packet
    /// Cancel-safe: This just receives from a [`Receiver`]
    pub async fn receive(&mut self) -> PacketBin<N> {
        self.receiver.receive().await
    }

    pub fn try_receive(&mut self) -> Option<PacketBin<N>> {
        self.receiver.try_receive().ok()
    }

    /// Encode a [`Packet`] as [`PacketBin`], and send via [`Self::send`].
    /// Cancel-safe: This just performs encoding without side-effects and
    /// then calls through to cancel-safe send.
    pub async fn send_packet<P>(&mut self, packet: &P) -> Result<(), ClientError>
    where
        P: Packet + write::Write,
    {
        let mut buf = [0; N];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf);
            r.put(packet)?;
            r.position()
        };
        let packet = PacketBin { buf, len };
        buf[0] = 1;
        self.send(packet).await;
        Ok(())
    }

    pub async fn send_packet_timeout<PP>(
        &mut self,
        packet: &PP,
        duration: Duration,
    ) -> Result<(), ClientError>
    where
        PP: Packet + write::Write,
    {
        self.send_packet(packet)
            .with_timeout(duration)
            .await
            .map_err(|_| ClientError::PacketWrite(PacketWriteError::ConnectionSend))??;
        Ok(())
    }

    pub async fn send_timeout(
        &mut self,
        message: PacketBin<N>,
        duration: Duration,
    ) -> Result<(), ClientError> {
        self.send(message)
            .with_timeout(duration)
            .await
            .map_err(|_| ClientError::PacketWrite(PacketWriteError::ConnectionSend))?;
        Ok(())
    }
}
