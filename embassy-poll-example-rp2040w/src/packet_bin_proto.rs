use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Instant, Timer};
use mountain_mqtt::client::ClientError;
use mountain_mqtt::client::ConnectionSettings;
use mountain_mqtt::data::quality_of_service::QualityOfService;
use mountain_mqtt_embassy::poll_client::{self, PollClient, Settings};
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
        let packet_bin = client.receive().await?;
        let event = client.process(&packet_bin).await?;
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
        let packet_bin = client.receive().await?;
        let event = client.process(&packet_bin).await?;
        info!("Event: {:?}", event);
    }

    // Poll for packets for 20 seconds, then disconnect
    let end_time = Instant::now() + Duration::from_secs(20);
    loop {
        match select(client.receive(), Timer::at(end_time)).await {
            Either::First(packet_bin) => {
                let packet_bin = packet_bin?;
                let event = client.process(&packet_bin).await?;
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

// TODO: Move to accepting a trait impl rather than AsyncFn, so it's easier to package up say some
// queues and provide an async method to run with them?
pub async fn run(settings: Settings, stack: Stack<'static>) {
    loop {
        info!("run: Trying MQTT connection");

        if let Err(e) = poll_client::run_mqtt_connection(settings, stack, demo_poll_result).await {
            info!("run: Error {}, will reconnect", e);
        }

        // Wait a while to try reconnecting
        Timer::after(Duration::from_secs(2)).await;
    }
}
