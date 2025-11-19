use defmt::*;
use embassy_futures::join::join3;
use embassy_futures::select::{select, Either};
use embassy_net::tcp::TcpSocket;
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::pubsub::PubSubChannel;
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

        let cancel_pubsub = PubSubChannel::<NoopRawMutex, bool, 1, 3, 3>::new();
        let rx_cancel_pub = cancel_pubsub.publisher().expect("rx_cancel_pub");
        let tx_cancel_pub = cancel_pubsub.publisher().expect("tx_cancel_pub");
        let poll_cancel_pub = cancel_pubsub.publisher().expect("poll_cancel_pub");

        let mut rx_cancel_sub = cancel_pubsub.subscriber().expect("rx_cancel_sub");
        let mut tx_cancel_sub = cancel_pubsub.subscriber().expect("tx_cancel_sub");
        let mut poll_cancel_sub = cancel_pubsub.subscriber().expect("poll_cancel_sub");

        let (mut rx, mut tx) = socket.split();

        let rx_fut = async {
            let mut buf = [0; 8];
            info!("rx_fut: Starting");

            loop {
                info!("rx_fut: Select on read_exact/cancel");
                match select(rx.read_exact(&mut buf), rx_cancel_sub.next_message_pure()).await {
                    Either::First(read) => match read {
                        Ok(_) => {
                            info!("rx_fut: Have read data {:?}, select on send/cancel", &buf);
                            match select(
                                rx_channel_sender.send(buf),
                                rx_cancel_sub.next_message_pure(),
                            )
                            .await
                            {
                                Either::First(_) => {
                                    info!("rx_fut: Sent data {:?} to channel", &buf)
                                }
                                Either::Second(_) => {
                                    info!("rx_fut: received cancellation (while sending to channel), will exit");
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            info!("rx_fut: Read error {}, stopping reading", e);
                            if let Err(e) = rx_cancel_pub.try_publish(true) {
                                info!("rx_fut: failed to cancel - already cancelled '{}'", e);
                            } else {
                                info!("rx_fut: cancelled");
                            }
                            break;
                        }
                    },
                    Either::Second(_) => {
                        info!(
                            "rx_fut: received cancellation (while reading from network), will exit"
                        );
                        break;
                    }
                }
            }
        };

        let tx_fut = async {
            info!("tx_fut: Starting");
            loop {
                info!("tx_fut: Select message");

                match select(
                    tx_channel_receiver.receive(),
                    tx_cancel_sub.next_message_pure(),
                )
                .await
                {
                    Either::First(write) => {
                        info!("tx_fut: Will write {:?}", &write);
                        match select(tx.write_all(&write), tx_cancel_sub.next_message_pure()).await
                        {
                            Either::First(write) => match write {
                                Ok(_) => {
                                    info!("txfut:...wrote data {:?}", &write);
                                }
                                Err(e) => {
                                    info!("txfut:...write error {}, stopping writing", e);
                                    if let Err(e) = tx_cancel_pub.try_publish(true) {
                                        info!(
                                            "tx_fut: failed to cancel - already cancelled '{}'",
                                            e
                                        );
                                    } else {
                                        info!("tx_fut: cancelled");
                                    }
                                    break;
                                }
                            },
                            Either::Second(_) => {
                                info!(
                                    "tx_fut: received cancellation (during write_all), will exit"
                                );
                                break;
                            }
                        }
                    }
                    Either::Second(_) => {
                        info!("tx_fut: received cancellation (during channel receive), will exit");
                        break;
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

                match select(
                    tx_channel_sender.send(index.to_ne_bytes()),
                    poll_cancel_sub.next_message_pure(),
                )
                .await
                {
                    Either::First(_) => {
                        info!("poll_fut: Sent data ({}){:?}", &index, &index.to_ne_bytes());
                    }
                    Either::Second(_) => {
                        info!("poll_fut: received cancellation (during sending data), will break");
                        break;
                    }
                }

                info!("poll_fut: About to receive data (or cancel)");

                match select(
                    rx_channel_receiver.receive(),
                    poll_cancel_sub.next_message_pure(),
                )
                .await
                {
                    Either::First(received) => {
                        info!("poll_fut: Received data {:?} from channel", &received);
                    }
                    Either::Second(_) => {
                        info!(
                            "poll_fut: received cancellation (during receiving data), will break"
                        );
                        break;
                    }
                }

                Delay.delay_ms(1000).await;

                index += 1;

                if index >= 10 {
                    info!("poll_fut: Enough data sent, cancelling...");
                    if let Err(e) = poll_cancel_pub.try_publish(true) {
                        info!("poll_fut: failed to cancel - already cancelled '{}'", e);
                    } else {
                        info!("poll_fut: cancelled");
                    }
                    break;
                }
            }
        };

        info!("About to start tcp futures");

        join3(rx_fut, tx_fut, poll_fut).await;

        info!("rx and tx futures finished");

        info!("Finished network comms, will reconnect");
    }
}
