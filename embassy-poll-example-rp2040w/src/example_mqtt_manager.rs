use crate::action::Action;
use crate::channels::{ActionSub, EventPub};
use crate::event::{Event, TOPIC_LED};
use defmt::*;
use embassy_executor::Spawner;
use embassy_futures::select::{self, Either};
use embassy_net::{Ipv4Address, Stack};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use heapless::String;
use mountain_mqtt::client::{Client, ClientError, ConnectionSettings};
use mountain_mqtt::data::quality_of_service::QualityOfService;
use mountain_mqtt::mqtt_manager::{ConnectionId, MqttOperations};
use mountain_mqtt_embassy::mqtt_manager::{self, MqttEvent, Settings};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

pub const TOPIC_ANNOUNCE: &str = "embassy-example-rp2040w-presence";

#[derive(Clone)]
pub enum MqttAction {
    AnnounceAndSubscribe { connection_id: ConnectionId },
    Action(Action),
}

impl MqttOperations for MqttAction {
    async fn perform<'a, 'b, C>(
        &'b mut self,
        client: &mut C,
        client_id: &'a str,
        current_connection_id: ConnectionId,
        is_retry: bool,
    ) -> Result<(), ClientError>
    where
        C: Client<'a>,
    {
        match self {
            // Specific to one connection, not retried
            Self::AnnounceAndSubscribe { connection_id } => {
                if connection_id == &current_connection_id && !is_retry {
                    client
                        .publish(
                            TOPIC_ANNOUNCE,
                            "true".as_bytes(),
                            QualityOfService::Qos1,
                            false,
                        )
                        .await?;
                    client.subscribe(TOPIC_LED, QualityOfService::Qos1).await?;
                }
            }
            // Actions are sent on any connection, and retried
            Self::Action(action) => {
                action
                    .perform(client, client_id, current_connection_id, is_retry)
                    .await?;
            }
        }
        Ok(())
    }
}

#[embassy_executor::task]
async fn mqtt_channel_task(
    stack: Stack<'static>,
    uid: &'static str,
    event_sender: Sender<'static, NoopRawMutex, MqttEvent<Event>, 32>,
    action_receiver: Receiver<'static, NoopRawMutex, MqttAction, 32>,
    host: Ipv4Address,
    port: u16,
) -> ! {
    let settings = Settings::new(host, port);
    let connection_settings = ConnectionSettings::unauthenticated(uid);

    mqtt_manager::run::<MqttAction, Event, 16, 4096, 32>(
        stack,
        connection_settings,
        settings,
        event_sender,
        action_receiver,
    )
    .await;
}

#[embassy_executor::task]
async fn mqtt_task(
    mut actions_in: ActionSub,
    actions_out: Sender<'static, NoopRawMutex, MqttAction, 32>,
    events_in: Receiver<'static, NoopRawMutex, MqttEvent<Event>, 32>,
    events_out: EventPub,
) -> ! {
    loop {
        let next = select::select(actions_in.next_message_pure(), events_in.receive()).await;
        match next {
            Either::First(action) => {
                // Always leave space free for sending AnnounceAndSubscribe actions
                // in response to connecting - if we don't send these, we won't subscribe, and
                // the MQTT connection won't work as expected
                // If we have to drop outgoing actions, do so rather than blocking
                if actions_out.free_capacity() > 8 {
                    let _ = actions_out.try_send(MqttAction::Action(action));
                }
            }
            Either::Second(event) => match event {
                MqttEvent::ApplicationEvent {
                    connection_id: _,
                    event,
                } => {
                    events_out.publish_immediate(event);
                }
                MqttEvent::Connected { connection_id } => {
                    actions_out
                        .send(MqttAction::AnnounceAndSubscribe { connection_id })
                        .await
                }
                MqttEvent::ConnectionStable { .. } => info!("MQTT connection stable"),
                MqttEvent::Disconnected { .. } => info!("MQTT disconnected"),
                event => {
                    info!("{:?}", event);
                }
            },
        }
    }
}

static EVENT_CHANNEL: StaticCell<Channel<NoopRawMutex, MqttAction, 32>> = StaticCell::new();
static ACTION_CHANNEL: StaticCell<Channel<NoopRawMutex, MqttEvent<Event>, 32>> = StaticCell::new();

pub async fn init(
    spawner: &Spawner,
    stack: Stack<'static>,
    uid: &'static String<64>,
    event_pub: EventPub,
    action_sub: ActionSub,
    host: Ipv4Address,
    port: u16,
) {
    let mqtt_action_channel = EVENT_CHANNEL.init(Channel::<NoopRawMutex, MqttAction, 32>::new());
    let mqtt_event_channel =
        ACTION_CHANNEL.init(Channel::<NoopRawMutex, MqttEvent<Event>, 32>::new());

    unwrap!(spawner.spawn(mqtt_channel_task(
        stack,
        uid,
        mqtt_event_channel.sender(),
        mqtt_action_channel.receiver(),
        host,
        port
    )));

    unwrap!(spawner.spawn(mqtt_task(
        action_sub,
        mqtt_action_channel.sender(),
        mqtt_event_channel.receiver(),
        event_pub,
    )));
}
