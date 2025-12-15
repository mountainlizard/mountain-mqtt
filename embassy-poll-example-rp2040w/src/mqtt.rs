use crate::action::Action;
use crate::channels::ActionSub;
use crate::channels::EventPub;
use crate::event::Event;
use defmt::*;
use embassy_futures::select::{select, Either};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_time::{Duration, Timer};
use mountain_mqtt::client::Client as _;
use mountain_mqtt::client::{
    ClientError, ClientReceivedEvent, ConnectionSettings, EventHandlerError,
};
use mountain_mqtt::{client_state::ClientStateNoQueue, data::quality_of_service::QualityOfService};
use mountain_mqtt_embassy::handler_client::SyncEventHandler;
use mountain_mqtt_embassy::mqtt_manager::FromApplicationMessage;
use mountain_mqtt_embassy::poll_client::{self, PollClient, Settings};
use {defmt_rtt as _, panic_probe as _};

pub const TOPIC_ANNOUNCE: &str = "embassy-poll-example-rp2040w-presence";
pub const TOPIC_LED: &str = "embassy-poll-example-rp2040w-led";
pub const TOPIC_BUTTON: &str = "embassy-poll-example-rp2040w-button";

// Type defined just to avoid repeating the parameters of PollClient
type OurPollClient<'a> = PollClient<'a, ClientStateNoQueue, NoopRawMutex, 1024, 16>;

pub struct QueueEventHandler<'a, const P: usize> {
    event_pub: &'a mut EventPub,
}

impl<'a, const P: usize> SyncEventHandler<P> for QueueEventHandler<'a, P> {
    fn handle_event(&mut self, event: ClientReceivedEvent<P>) -> Result<(), EventHandlerError> {
        match event {
            mountain_mqtt::client::ClientReceivedEvent::ApplicationMessage(message) => {
                let event = Event::from_application_message(&message)?;
                if let Err(e) = self.event_pub.try_publish(event) {
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
}

pub async fn client_function_with_channels(
    mut client: OurPollClient<'_>,
    uid: &'static str,
    event_pub: &mut EventPub,
    action_sub: &mut ActionSub,
) -> Result<(), ClientError> {
    let handler = QueueEventHandler { event_pub };

    // Connect - this sends packet and then waits for response
    client
        .connect(&ConnectionSettings::unauthenticated(uid))
        .await?;

    let mut client = client.to_handler_client(handler);

    // Subscribe - this sends packet but does NOT wait for response - we will need to poll for packets
    client.subscribe(TOPIC_LED, QualityOfService::Qos1).await?;

    // Announce ourselves
    client
        .publish(
            TOPIC_ANNOUNCE,
            "true".as_bytes(),
            QualityOfService::Qos1,
            false,
        )
        .await?;

    // Poll for incoming MQTT application messages, and for actions we need
    // to send out to MQTT server
    loop {
        match select(action_sub.next_message_pure(), client.receive()).await {
            Either::First(action) => match action {
                Action::Button(pressed) => {
                    let payload = if pressed { "true" } else { "false" };
                    client
                        .publish(
                            TOPIC_BUTTON,
                            payload.as_bytes(),
                            QualityOfService::Qos1,
                            false,
                        )
                        .await?;
                }
            },
            // Errors from handler are returned causing reconnection
            Either::Second(result) => result?,
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
        info!("run: Trying MQTT connection, will use handler");

        // `poll_client::run_mqtt_connection` accepts an async function that just accepts a `PollClient` and
        // uses it to manage MQTT interactions.
        // Since we want to use `event_pub` and `action_sub` as well, we capture them in a closure in this
        // anonymous async function, and we can then pass that to run the connection.
        let client_function = async |client: OurPollClient<'_>| {
            client_function_with_channels(client, uid, &mut event_pub, &mut action_sub).await
        };

        if let Err(e) = poll_client::run_mqtt_connection(settings, stack, client_function).await {
            info!("run: Error {}, will reconnect", e);
        }

        // Wait a while to try reconnecting
        Timer::after(Duration::from_secs(2)).await;
    }
}
