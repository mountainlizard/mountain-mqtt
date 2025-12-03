use crate::action::Action;
use crate::channels::ActionSub;
use crate::channels::EventPub;
use crate::event::Event;
use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Timer};
use mountain_mqtt::client::ClientError;
use mountain_mqtt::client::ConnectionSettings;
use mountain_mqtt::data::quality_of_service::QualityOfService;
use mountain_mqtt_embassy::mqtt_manager::FromApplicationMessage;
use mountain_mqtt_embassy::packet_bin::PacketBin;
use mountain_mqtt_embassy::poll_client::{self, PollClient, Settings};

use {defmt_rtt as _, panic_probe as _};

pub const TOPIC_ANNOUNCE: &str = "embassy-example-rp2040w-presence";
pub const TOPIC_LED: &str = "embassy-example-rp2040w-led";
pub const TOPIC_BUTTON: &str = "embassy-example-rp2040w-button";

pub async fn send_action(
    client: &mut PollClient<'_, NoopRawMutex, 1024, 16>,
    action: &Action,
) -> Result<(), ClientError> {
    match action {
        Action::Button(pressed) => {
            let payload = if *pressed { "true" } else { "false" };
            client
                .publish(
                    TOPIC_BUTTON,
                    payload.as_bytes(),
                    QualityOfService::Qos1,
                    false,
                )
                .await?;
        }
    }
    Ok(())
}

pub async fn receive_event(
    client: &mut PollClient<'_, NoopRawMutex, 1024, 16>,
    packet_bin: PacketBin<1024>,
    event_pub: &mut EventPub,
) -> Result<(), ClientError> {
    let event = client.process(&packet_bin).await?;
    match event {
        mountain_mqtt::client::ClientReceivedEvent::ApplicationMessage(message) => {
            let event = Event::from_application_message(&message)?;
            if let Err(e) = event_pub.try_publish(event) {
                // Warn on overflow - we could also return a ClientError to cancel polling
                warn!("Overflow in event channel, dropping event {:?}", e)
            }
        }
        mountain_mqtt::client::ClientReceivedEvent::Ack => info!("Received Ack"),
        mountain_mqtt::client::ClientReceivedEvent::SubscriptionGrantedBelowMaximumQos {
            granted_qos,
            maximum_qos,
        } => warn!(
            "SubscriptionGrantedBelowMaximumQos, requested {:?}, received {:?}",
            maximum_qos, granted_qos
        ),
        mountain_mqtt::client::ClientReceivedEvent::PublishedMessageHadNoMatchingSubscribers => {
            warn!("PublishedMessageHadNoMatchingSubscribers")
        }
        mountain_mqtt::client::ClientReceivedEvent::NoSubscriptionExisted => {
            warn!("NoSubscriptionExisted")
        }
    }
    Ok(())
}

pub async fn wait_for_responses(
    client: &mut PollClient<'_, NoopRawMutex, 1024, 16>,
    event_pub: &mut EventPub,
) -> Result<(), ClientError> {
    while client.waiting_for_responses() {
        let packet_bin = client.receive().await?;
        receive_event(client, packet_bin, event_pub).await?;
    }
    Ok(())
}

pub async fn demo_poll_result(
    client: &mut PollClient<'_, NoopRawMutex, 1024, 16>,
    uid: &'static str,
    event_pub: &mut EventPub,
    action_sub: &mut ActionSub,
) -> Result<(), ClientError> {
    // Connect - this sends packet and then waits for response
    client
        .connect(&ConnectionSettings::unauthenticated(uid))
        .await?;

    // Subscribe - this sends packet but does NOT wait for response - we will need to poll for packets
    client.subscribe(TOPIC_LED, QualityOfService::Qos1).await?;
    wait_for_responses(client, event_pub).await?;

    // Announce ourselves
    client
        .publish(
            TOPIC_ANNOUNCE,
            "true".as_bytes(),
            QualityOfService::Qos1,
            false,
        )
        .await?;
    wait_for_responses(client, event_pub).await?;

    // Poll for incoming MQTT application messages, and for actions we need
    // to send out to MQTT server
    loop {
        match select(action_sub.next_message_pure(), client.receive()).await {
            Either::First(action) => {
                send_action(client, &action).await?;
                wait_for_responses(client, event_pub).await?;
            }
            Either::Second(packet_bin) => {
                let packet_bin = packet_bin?;
                receive_event(client, packet_bin, event_pub).await?;
            }
        }
    }
}

#[embassy_executor::task]
pub async fn run(
    settings: Settings,
    stack: Stack<'static>,
    uid: &'static str,
    mut event_pub: EventPub,
    mut action_sub: ActionSub,
) {
    loop {
        info!("run: Trying MQTT connection");

        // `poll_client::run_mqtt_connection` accepts an async function that just accepts a `PollClient` and
        // uses it to manage MQTT interactions.
        // Since we want to use `event_pub` and `action_sub` as well, we capture them in a closure in this
        // anonymous async function, and we can then pass that to run the connection.
        let poll = async |client: &mut PollClient<'_, NoopRawMutex, 1024, 16>| {
            demo_poll_result(client, uid, &mut event_pub, &mut action_sub).await
        };

        if let Err(e) = poll_client::run_mqtt_connection(settings, stack, poll).await {
            info!("run: Error {}, will reconnect", e);
        }

        // Wait a while to try reconnecting
        Timer::after(Duration::from_secs(2)).await;
    }
}
