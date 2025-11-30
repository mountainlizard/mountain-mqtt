use crate::packet_bin;
use crate::packet_bin::PacketBin;
use crate::poll_client;
use crate::poll_client::PollClient;
use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_futures::select::{select3, Either3};
use embassy_net::tcp::TcpSocket;
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use embedded_io_async::Write;
use mountain_mqtt::client::ClientError;
use mountain_mqtt::client::ConnectionSettings;
use mountain_mqtt::data::quality_of_service::QualityOfService;
use mountain_mqtt_embassy::mqtt_manager::Settings;
use {defmt_rtt as _, panic_probe as _};

pub const TOPIC_ANNOUNCE: &str = "embassy-example-rp2040w-presence";
pub const TOPIC_LED: &str = "embassy-example-rp2040w-led";

pub async fn demo_poll_result(
    client: &mut PollClient<'_, NoopRawMutex, 1024, 16>,
) -> Result<(), ClientError> {
    // Connect - this sends packet and then waits for response
    client
        .connect(&ConnectionSettings::unauthenticated("packet_bin_proto"))
        .await?;

    // Subscribe - this sends packet but does NOT wait for response - we will need to poll for packets
    client.subscribe(TOPIC_LED, QualityOfService::Qos1).await?;

    // Poll for packets until response
    while client.waiting_for_responses() {
        let packet_bin = client.receive_bin().await?;
        let event = client.handle_packet_bin(&packet_bin).await?;
        info!("Event: {:?}", event);
    }

    client
        .publish(
            TOPIC_ANNOUNCE,
            "true".as_bytes(),
            QualityOfService::Qos1,
            false,
        )
        .await?;

    // Poll for packets until response
    while client.waiting_for_responses() {
        let packet_bin = client.receive_bin().await?;
        let event = client.handle_packet_bin(&packet_bin).await?;
        info!("Event: {:?}", event);
    }

    // Poll for packets for 20 seconds, then disconnect
    let end_time = Instant::now() + Duration::from_secs(20);
    loop {
        match select(client.receive_bin(), Timer::at(end_time)).await {
            Either::First(packet_bin) => {
                let packet_bin = packet_bin?;
                let event = client.handle_packet_bin(&packet_bin).await?;
                info!("Event: {:?}", event);
            }
            Either::Second(_) => {
                info!("Finished polling loop - will disconnect");
                break;
            }
        }

        // Poll without timeout
        // let packet_bin = client.receive_bin().await?;
        // let event = client.handle_packet_bin(&packet_bin).await?;
        // info!("Event: {:?}", event);
    }

    client.disconnect().await?;

    Ok(())
}

pub async fn demo_poll(client: &mut PollClient<'_, NoopRawMutex, 1024, 16>) {
    if let Err(e) = demo_poll_result(client).await {
        info!("demo_poll: Error {}", e);
    }
}

pub async fn run_with_demo_poll(settings: Settings, stack: Stack<'static>) {
    run(settings, stack, demo_poll).await
}

// TODO: Move to accepting a trait impl rather than AsyncFn, so it's easier to package up say some
// queues and provide an async method to run with them?
pub async fn run<M, const N: usize, const P: usize>(
    settings: Settings,
    stack: Stack<'static>,
    f: impl AsyncFn(&mut PollClient<M, N, P>),
) where
    M: RawMutex,
{
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

        let rx_channel: Channel<M, PacketBin<N>, 1> = Channel::new();
        let rx_channel_sender = rx_channel.sender();

        let tx_channel: Channel<M, PacketBin<N>, 1> = Channel::new();
        let tx_channel_receiver = tx_channel.receiver();

        let (mut rx, mut tx) = socket.split();

        let rx_fut = async {
            loop {
                match packet_bin::receive_packet_bin(&mut rx).await {
                    Ok(packet_bin) => rx_channel_sender.send(packet_bin).await,
                    Err(e) => return e,
                }
            }
        };

        let tx_fut = async {
            loop {
                let write = tx_channel_receiver.receive().await;
                // Ignore packets with length 0 - we can use these as a way to flush
                // the buffer.
                if write.len > 0 {
                    if let Err(e) = tx.write_all(write.msg_data()).await {
                        return e;
                    }
                }
            }
        };

        let mut client = PollClient::new(
            tx_channel.sender(),
            rx_channel.receiver(),
            poll_client::Settings::default(),
        );

        info!("About to start tcp futures");

        match select3(rx_fut, tx_fut, f(&mut client)).await {
            Either3::First(e) => warn!("Finished network comms with read error {:?}", e),
            Either3::Second(e) => warn!("Finished network comms with write error {:?}", e),
            Either3::Third(_) => info!("Finished network comms by polling completing"),
        }

        info!("Finished network comms, will reconnect");
    }
}
