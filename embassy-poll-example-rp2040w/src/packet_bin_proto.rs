use crate::packet_io;
use crate::packet_io::PacketBin;
use defmt::*;
use embassy_futures::select::{select3, Either3};
use embassy_net::tcp::TcpSocket;
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::channel::Receiver;
use embassy_sync::channel::Sender;
use embassy_time::{Delay, Timer};
use embedded_hal_async::delay::DelayNs;
use embedded_io_async::Write;
use mountain_mqtt::codec::mqtt_writer::{MqttBufWriter, MqttWriter};
use mountain_mqtt::codec::write;
use mountain_mqtt::error::PacketWriteError;
use mountain_mqtt::packets::connect::Connect;
use mountain_mqtt::packets::packet::Packet;
use mountain_mqtt_embassy::mqtt_manager::Settings;

use {defmt_rtt as _, panic_probe as _};

pub struct Client<'a, const N: usize> {
    sender: Sender<'a, NoopRawMutex, PacketBin<N>, 1>,
    receiver: Receiver<'a, NoopRawMutex, PacketBin<N>, 1>,
}

impl<'a, const N: usize> Client<'a, N> {
    pub async fn send_bin(&mut self, message: PacketBin<N>) {
        self.sender.send(message).await
    }

    pub async fn receive_bin(&mut self) -> PacketBin<N> {
        self.receiver.receive().await
    }

    pub async fn send<P>(&mut self, packet: P) -> Result<(), PacketWriteError>
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

pub async fn demo_poll(client: &mut Client<'_, 1024>) {
    let mut index: u64 = 0;
    loop {
        info!("demo_poll: About to send packet {}", &index,);

        let packet = Connect::unauthenticated_no_topic_aliases("packet_bin_proto");

        if let Err(e) = client.send(packet).await {
            info!("demo_poll: Failed to send connect packet {:?}", e);
        } else {
            info!("demo_poll: Sent connect packet");
        }

        info!("demo_poll: About to receive data (or cancel)");

        let received = client.receive_bin().await;
        info!(
            "demo_poll: Received data {:?} from channel",
            &received.msg_data()
        );

        Delay.delay_ms(1000).await;

        index += 1;

        if index >= 10 {
            info!("demo_poll: Enough data sent, ending...");
            return;
        }
    }
}

pub async fn run_with_demo_poll(settings: Settings, stack: Stack<'static>) {
    run(settings, stack, demo_poll).await
}

// TODO: Move to accepting a trait impl rather than AsyncFn, so it's easier to package up say some
// queues and provide an async method to run with them?
pub async fn run<const N: usize>(
    settings: Settings,
    stack: Stack<'static>,
    f: impl AsyncFn(&mut Client<N>),
) {
    let mut rx_buffer = [0; N];
    let mut tx_buffer = [0; N];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(None);

        let remote_endpoint = (settings.address, settings.port);
        info!("MQTT socket connecting to {:?}...", remote_endpoint);
        // TODO: This should just return directly, to let caller decide whether to retry
        if let Err(e) = socket.connect(remote_endpoint).await {
            warn!("MQTT socket connect error, will retry: {:?}", e);
            // Wait a while to try reconnecting
            Timer::after(settings.reconnection_delay).await;
            continue;
        }
        info!("MQTT socket connected!");

        let rx_channel: Channel<NoopRawMutex, PacketBin<N>, 1> = Channel::new();
        let rx_channel_sender = rx_channel.sender();

        let tx_channel: Channel<NoopRawMutex, PacketBin<N>, 1> = Channel::new();
        let tx_channel_receiver = tx_channel.receiver();

        let (mut rx, mut tx) = socket.split();

        let rx_fut = async {
            // let mut buf = [0; N];

            loop {
                match packet_io::receive_packet_bin(&mut rx).await {
                    Ok(packet_bin) => rx_channel_sender.send(packet_bin).await,
                    Err(e) => return e,
                }

                // if let Err(e) = rx.read_exact(&mut buf).await {
                //     return e;
                // }
                // rx_channel_sender
                //     .send(
                //         PacketBin::new(&buf)
                //             .expect("PacketBin size mismatch in rx_fut, should not occur"),
                //     )
                //     .await
            }
        };

        let tx_fut = async {
            loop {
                let write = tx_channel_receiver.receive().await;
                if let Err(e) = tx.write_all(write.msg_data()).await {
                    return e;
                }
            }
        };

        let mut client = Client {
            sender: tx_channel.sender(),
            receiver: rx_channel.receiver(),
        };

        info!("About to start tcp futures");

        match select3(rx_fut, tx_fut, f(&mut client)).await {
            Either3::First(e) => info!("Finished network comms with read error {:?}", e),
            Either3::Second(e) => info!("Finished network comms with write error {:?}", e),
            Either3::Third(_) => info!("Finished network comms by polling completing"),
        }

        info!("Finished network comms, will reconnect");
    }
}
