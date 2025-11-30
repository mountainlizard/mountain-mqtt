use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::{Receiver, Sender},
};
use mountain_mqtt::{
    client::ClientError,
    codec::{
        mqtt_writer::{MqttBufWriter, MqttWriter},
        write,
    },
    packets::packet::Packet,
};

use crate::packet_bin::PacketBin;

pub struct RawClient<'a, M, const N: usize>
where
    M: RawMutex,
{
    sender: Sender<'a, M, PacketBin<N>, 1>,
    receiver: Receiver<'a, M, PacketBin<N>, 1>,
}

impl<'a, M, const N: usize> RawClient<'a, M, N>
where
    M: RawMutex,
{
    pub fn new(
        sender: Sender<'a, M, PacketBin<N>, 1>,
        receiver: Receiver<'a, M, PacketBin<N>, 1>,
    ) -> Self {
        Self { sender, receiver }
    }

    pub async fn send_bin(&mut self, message: PacketBin<N>) {
        self.sender.send(message).await
    }

    pub async fn receive_bin(&mut self) -> PacketBin<N> {
        self.receiver.receive().await
    }

    pub fn try_receive_bin(&mut self) -> Option<PacketBin<N>> {
        self.receiver.try_receive().ok()
    }

    pub async fn send<P>(&mut self, packet: P) -> Result<(), ClientError>
    where
        P: Packet + write::Write,
    {
        let mut buf = [0; N];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf);
            r.put(&packet)?;
            r.position()
        };
        let packet = PacketBin { buf, len };
        buf[0] = 1;
        self.send_bin(packet).await;
        Ok(())
    }
}
