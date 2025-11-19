use defmt::*;
use embassy_futures::select::{select3, Either3};
use embassy_net::tcp::TcpSocket;
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Delay, Timer};
use embedded_hal_async::delay::DelayNs;
use embedded_io_async::Read;
use embedded_io_async::Write;
use mountain_mqtt_embassy::mqtt_manager::Settings;
use {defmt_rtt as _, panic_probe as _};

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
        let rx_channel_receiver = rx_channel.receiver();

        let tx_channel: Channel<NoopRawMutex, [u8; 8], 1> = Channel::new();
        let tx_channel_sender = tx_channel.sender();
        let tx_channel_receiver = tx_channel.receiver();

        let (mut rx, mut tx) = socket.split();

        let rx_fut = async {
            let mut buf = [0; 8];
            info!("rx_fut: Starting");

            loop {
                info!("rx_fut: read_exact");
                match rx.read_exact(&mut buf).await {
                    Ok(_) => {
                        info!("rx_fut: Have read data {:?}, select on send/cancel", &buf);
                        rx_channel_sender.send(buf).await;
                        info!("rx_fut: Sent data {:?} to channel", &buf)
                    }
                    Err(e) => {
                        info!("rx_fut: Read error {}, stopping reading", e);
                        return e;
                    }
                }
            }
        };

        let tx_fut = async {
            info!("tx_fut: Starting");
            loop {
                info!("tx_fut: receive message");

                let write = tx_channel_receiver.receive().await;
                info!("tx_fut: Will write {:?}", &write);

                match tx.write_all(&write).await {
                    Ok(_) => {
                        info!("txfut:...wrote data {:?}", &write);
                    }
                    Err(e) => {
                        info!("txfut:...write error {}, stopping writing", e);
                        return e;
                    }
                }
            }
        };

        let poll_fut = async {
            let mut index: u64 = 0;
            loop {
                info!(
                    "poll_fut: About to send data ({}){:?} (or cancel)",
                    &index,
                    &index.to_ne_bytes()
                );

                tx_channel_sender.send(index.to_ne_bytes()).await;
                info!("poll_fut: Sent data ({}){:?}", &index, &index.to_ne_bytes());

                info!("poll_fut: About to receive data (or cancel)");

                let received = rx_channel_receiver.receive().await;
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
