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
use embedded_io_async::Read;
use embedded_io_async::Write;
use mountain_mqtt_embassy::mqtt_manager::Settings;
use {defmt_rtt as _, panic_probe as _};

pub struct Client<'a> {
    sender: Sender<'a, NoopRawMutex, [u8; 8], 1>,
    receiver: Receiver<'a, NoopRawMutex, [u8; 8], 1>,
}

impl<'a> Client<'a> {
    pub async fn send(&mut self, message: [u8; 8]) {
        self.sender.send(message).await
    }

    pub async fn receive(&mut self) -> [u8; 8] {
        self.receiver.receive().await
    }
}

pub async fn run(settings: Settings, stack: Stack<'static>) {
    const B: usize = 1024;

    let mut rx_buffer = [0; B];
    let mut tx_buffer = [0; B];

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(None);

        let remote_endpoint = (settings.address, settings.port);
        info!("MQTT socket connecting to {:?}...", remote_endpoint);
        if let Err(e) = socket.connect(remote_endpoint).await {
            warn!("MQTT socket connect error, will retry: {:?}", e);
            // Wait a while to try reconnecting
            Timer::after(settings.reconnection_delay).await;
            continue;
        }
        info!("MQTT socket connected!");

        // let connection = ConnectionEmbedded::new(socket);

        let rx_channel: Channel<NoopRawMutex, [u8; 8], 1> = Channel::new();
        let rx_channel_sender: embassy_sync::channel::Sender<'_, NoopRawMutex, [u8; 8], 1> =
            rx_channel.sender();
        // let rx_channel_receiver = rx_channel.receiver();

        let tx_channel: Channel<NoopRawMutex, [u8; 8], 1> = Channel::new();
        // let tx_channel_sender = tx_channel.sender();
        let tx_channel_receiver = tx_channel.receiver();

        let (mut rx, mut tx) = socket.split();

        let rx_fut = async {
            let mut buf = [0; 8];

            loop {
                if let Err(e) = rx.read_exact(&mut buf).await {
                    return e;
                }
                rx_channel_sender.send(buf).await
            }
        };

        let tx_fut = async {
            loop {
                let write = tx_channel_receiver.receive().await;
                if let Err(e) = tx.write_all(&write).await {
                    return e;
                }
            }
        };

        let mut client = Client {
            sender: tx_channel.sender(),
            receiver: rx_channel.receiver(),
        };

        let poll_fut = async {
            let mut index: u64 = 0;
            loop {
                info!(
                    "poll_fut: About to send data ({}){:?} (or cancel)",
                    &index,
                    &index.to_ne_bytes()
                );

                client.send(index.to_ne_bytes()).await;
                info!("poll_fut: Sent data ({}){:?}", &index, &index.to_ne_bytes());

                info!("poll_fut: About to receive data (or cancel)");

                let received = client.receive().await;
                info!("poll_fut: Received data {:?} from channel", &received);

                Delay.delay_ms(1000).await;

                index += 1;

                if index >= 10 {
                    info!("poll_fut: Enough data sent, ending...");
                    return;
                }
            }
        };

        info!("About to start tcp futures");

        match select3(rx_fut, tx_fut, poll_fut).await {
            Either3::First(e) => info!("Finished network comms with read error {:?}", e),
            Either3::Second(e) => info!("Finished network comms with write error {:?}", e),
            Either3::Third(_) => info!("Finished network comms by polling completing"),
        }

        info!("Finished network comms, will reconnect");
    }
}
