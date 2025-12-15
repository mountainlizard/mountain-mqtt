use core::cell::RefCell;
use embassy_net::{tcp::TcpSocket, Ipv4Address, Stack};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::{Receiver, Sender};
use embassy_time::{Delay, Duration, Instant, Timer};
use mountain_mqtt::client::{
    Client, ClientError, ClientNoQueue, ClientReceivedEvent, ConnectionSettings, EventHandler,
    EventHandlerError,
};
use mountain_mqtt::data::quality_of_service::QualityOfService;
use mountain_mqtt::embedded_hal_async::DelayEmbedded;
use mountain_mqtt::embedded_io_async::ConnectionEmbedded;
use mountain_mqtt::mqtt_manager::{ConnectionId, MqttOperations};
use mountain_mqtt::packets::publish::ApplicationMessage;

/// Convert an [ApplicationMessage] to an application-specific event type
/// This is a specific trait rather than [TryFrom] so it can use a specific
/// error type, and include the expected number of properties in the
/// [ApplicationMessage].
pub trait FromApplicationMessage<const P: usize>: Sized {
    fn from_application_message(message: &ApplicationMessage<P>)
        -> Result<Self, EventHandlerError>;
}

/// Represents an error while running MQTT connections
/// These are passed to the [MqttEvent] channel in
/// [MqttEvent::Disconnected] events
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Error {
    Client(ClientError),
    MqttServerUnresponsive,
}

impl From<ClientError> for Error {
    fn from(value: ClientError) -> Self {
        Self::Client(value)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Error {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Error::Client(client_error) => defmt::write!(f, "Client({})", client_error),
            Error::MqttServerUnresponsive => defmt::write!(f, "MqttServerUnresponsive"),
        }
    }
}

/// Settings for the manager, including the address and port of the server,
/// and the various timeouts and intervals used to manage sending pings,
/// monitoring whether connections are responsive, and when to report that
/// a connection is stabilised.
pub struct Settings {
    /// The address of the MQTT server
    pub address: Ipv4Address,

    /// The port of the MQTT server
    pub port: u16,

    /// The minimum interval between sending pings to the
    /// server. The actual ping interval might be longer
    /// due to the additional time spent between checks,
    /// which includes waiting for the poll_interval and any time
    /// spent receiving MQTT messages.
    pub ping_interval: Duration,

    /// The maximum interval permitted between connection events
    /// (receiving events from the server that indicate our connection
    /// is still live). This should be greater than the ping_interval
    /// by some margin - there may be no events other than ping responses,
    /// and pings may be lost or delayed.
    pub connection_event_max_interval: Duration,

    /// The delay between disconnecting after detecting a connection error,
    /// and reconnecting to MQTT server
    pub reconnection_delay: Duration,

    /// The additional delay applied for each MQTT loop
    pub poll_interval: Duration,

    /// The maximum time we will wait for a specific response to sending
    /// and MQTT packet to the server. This only applies to packets
    /// that expect a response, e.g. subscribe, unsubscribe, publish with qos 1 or higher.
    /// This can be shorter than the connection_event_max_interval since we expect the
    /// server to respond quickly to packets that require a response, essentially
    /// this is the maximum permitted round-trip time to send a packet to the
    /// server and receive back a response.
    pub response_timeout: Duration,

    /// [Handler::on_connection_stable] after the connection has been
    /// maintained for this long. This can be used for example to check
    /// that any expected retained messages have been received, and if they
    /// haven't they can be published.
    pub stabilisation_interval: Duration,
}

impl Settings {
    /// Create a new [Settings] with default intervals and timeouts
    pub fn new(address: Ipv4Address, port: u16) -> Self {
        Self {
            address,
            port,
            ping_interval: Duration::from_millis(2000),
            connection_event_max_interval: Duration::from_millis(10000),
            reconnection_delay: Duration::from_millis(2000),
            poll_interval: Duration::from_millis(10),
            response_timeout: Duration::from_millis(5000),
            stabilisation_interval: Duration::from_millis(5000),
        }
    }
}

/// When a relevant message is received from an MQTT connection, an [MqttEvent]
/// is sent to the event channel so the application can respond to it.
#[derive(Debug, Clone)]
pub enum MqttEvent<E> {
    /// The connection with specified [ConnectionId] received an [ApplicationMessage],
    /// which was converted to `event`
    ApplicationEvent {
        connection_id: ConnectionId,
        event: E,
    },

    /// A new connection has been made, with specified [ConnectionId]
    Connected { connection_id: ConnectionId },

    /// A new connection, with specified [ConnectionId], has been stable
    /// for the interval specified in [Settings].
    /// One use case for this is where you wish to ensure that a given
    /// topic has a retained message, but not to replace any existing
    /// retained message on that topic. To do this, subscribe to the
    /// topic as soon as possible after connection, by sending an action
    /// to subscribe in response to the [MqttEvent::Connected] event.
    /// Then if no [MqttEvent::ApplicationEvent] has been received from
    /// the topic before [MqttEvent::ConnectionStable], you can assume
    /// this means that no retained message exists - you can then send
    /// an action to publish a new retained message.
    ConnectionStable { connection_id: ConnectionId },

    /// The connection with specified [ConnectionId] has disconnected, with
    /// given error. Note that a new connection will be made automatically,
    /// in many cases the error will be recoverable, e.g. it may be caused
    /// by a network interruption, but if there are repeated errors the
    /// error type can be used for establishing the root cause.
    Disconnected {
        connection_id: ConnectionId,
        error: Error,
    },

    /// A subscription was granted but was at lower qos than the maximum requested
    /// This may or may not require action depending on client requirements -
    /// it means that the given subscription will receive published messages at
    /// only the granted qos - if the requested maximum qos was absolutely required
    /// then the client could respond by showing an error to the user stating the
    /// server is incompatible, or possibly trying to unsubscribe and resubscribe,
    /// assuming this is expected to make any difference with the server(s) in use.
    SubscriptionGrantedBelowMaximumQos {
        connection_id: ConnectionId,
        granted_qos: QualityOfService,
        maximum_qos: QualityOfService,
    },

    /// A published message was received at the server, but had no matching subscribers and
    /// so did not reach any receivers
    /// This may or may not require action depending on client requirements / expectations
    /// E.g. if it was expected there would be subscribers, the client could try resending
    /// the message later
    PublishedMessageHadNoMatchingSubscribers { connection_id: ConnectionId },

    // Server processed an unsubscribe request, but no such subscription existed on the server,
    // so nothing changed.
    /// This may or may not require action depending on client requirements / expectations
    /// E.g. if it was expected there would be a subscription, the client could produce
    /// an error, and the user of the client might try reconnecting to the server to set
    /// up subscriptions again.
    NoSubscriptionExisted { connection_id: ConnectionId },
}

#[cfg(feature = "defmt")]
impl<E> defmt::Format for MqttEvent<E> {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::ApplicationEvent {
                connection_id,
                event: _,
            } => defmt::write!(f, "ApplicationEvent({})", connection_id),
            Self::Connected { connection_id } => defmt::write!(f, "Connected({})", connection_id),
            Self::ConnectionStable { connection_id } => {
                defmt::write!(f, "ConnectionStable({})", connection_id)
            }
            Self::Disconnected {
                connection_id,
                error,
            } => defmt::write!(f, "Disconnected({}, {})", connection_id, error),
            Self::SubscriptionGrantedBelowMaximumQos {
                connection_id,
                granted_qos,
                maximum_qos,
            } => defmt::write!(
                f,
                "SubscriptionGrantedBelowMaximumQos({}, granted: {}, maximum: {})",
                connection_id,
                granted_qos,
                maximum_qos
            ),
            Self::PublishedMessageHadNoMatchingSubscribers { connection_id } => {
                defmt::write!(
                    f,
                    "PublishedMessageHadNoMatchingSubscribers({})",
                    connection_id
                )
            }
            Self::NoSubscriptionExisted { connection_id } => {
                defmt::write!(f, "NoSubscriptionExisted({})", connection_id)
            }
        }
    }
}

struct State<A> {
    /// The instant when the most recent connection event occurred, indicating
    /// the connection was live. Set when state is created
    /// (should be just after connecting), and again whenever we receive an
    /// Ack event indicating we received a packet from the server
    last_connection_event: Instant,

    /// A failed action that needs to be retried
    pub pending_action: Option<A>,
}

impl<A> State<A> {
    fn new() -> Self {
        Self {
            last_connection_event: Instant::now(),
            pending_action: None,
        }
    }
    fn record_connection_event(&mut self) {
        self.last_connection_event = Instant::now();
    }
}

struct ChannelEventHandler<'a, A, E, const P: usize, const Q: usize>
where
    E: FromApplicationMessage<P> + Clone,
{
    connection_id: ConnectionId,
    event_sender: &'a Sender<'a, NoopRawMutex, MqttEvent<E>, Q>,
    state: &'a RefCell<State<A>>,
}

impl<A, E, const P: usize, const Q: usize> EventHandler<P> for ChannelEventHandler<'_, A, E, P, Q>
where
    E: FromApplicationMessage<P> + Clone,
{
    async fn handle_event(
        &mut self,
        event: ClientReceivedEvent<'_, P>,
    ) -> Result<(), EventHandlerError> {
        match event {
            ClientReceivedEvent::ApplicationMessage(message) => {
                let event = E::from_application_message(&message)?;
                self.event_sender
                    .send(MqttEvent::ApplicationEvent {
                        connection_id: self.connection_id,
                        event,
                    })
                    .await;
            }
            ClientReceivedEvent::Ack => {
                self.state.borrow_mut().record_connection_event();
            }
            ClientReceivedEvent::SubscriptionGrantedBelowMaximumQos {
                granted_qos,
                maximum_qos,
            } => {
                self.event_sender
                    .send(MqttEvent::SubscriptionGrantedBelowMaximumQos {
                        connection_id: self.connection_id,
                        granted_qos,
                        maximum_qos,
                    })
                    .await
            }
            ClientReceivedEvent::PublishedMessageHadNoMatchingSubscribers => {
                self.event_sender
                    .send(MqttEvent::PublishedMessageHadNoMatchingSubscribers {
                        connection_id: self.connection_id,
                    })
                    .await
            }
            ClientReceivedEvent::NoSubscriptionExisted => {
                self.event_sender
                    .send(MqttEvent::NoSubscriptionExisted {
                        connection_id: self.connection_id,
                    })
                    .await
            }
        }
        Ok(())
    }
}

async fn try_action<'a, A, C>(
    current_connection_id: ConnectionId,
    client: &mut C,
    state: &RefCell<State<A>>,
    connection_settings: &ConnectionSettings<'static>,
    mut action: A,
    is_retry: bool,
) -> Result<(), ClientError>
where
    C: Client<'a>,
    A: MqttOperations + Clone,
{
    if let Err(e) = action
        .perform(
            client,
            connection_settings.client_id(),
            current_connection_id,
            is_retry,
        )
        .await
    {
        state.borrow_mut().pending_action = Some(action);
        return Err(e);
    }
    Ok(())
}

/// Handle messages until we encounter an error
async fn handle_messages<'a, A, C, E, const Q: usize>(
    current_connection_id: ConnectionId,
    client: &mut C,
    state: &RefCell<State<A>>,
    connection_settings: &ConnectionSettings<'static>,
    event_sender: &Sender<'static, NoopRawMutex, MqttEvent<E>, Q>,
    action_receiver: &mut Receiver<'static, NoopRawMutex, A, Q>,
    settings: &Settings,
) -> Result<(), Error>
where
    C: Client<'a>,
    A: MqttOperations + Clone,
    E: Clone,
{
    client.connect(connection_settings).await?;

    event_sender
        .send(MqttEvent::Connected {
            connection_id: current_connection_id,
        })
        .await;

    let mut connection_instant = Some(Instant::now());
    let mut last_ping_instant = Instant::now();

    // TODO: This should use a select (on ping due, stabilisation elapsed,
    // connection event check needed, client poll and action_receiver.receive),
    // but needs some work on the client `poll` method first
    loop {
        Timer::after(settings.poll_interval).await;

        if last_ping_instant.elapsed() > settings.ping_interval {
            last_ping_instant = Instant::now();
            client.send_ping().await?;
        }

        // Check for stabilisation
        if let Some(instant) = connection_instant {
            if instant.elapsed() > settings.stabilisation_interval {
                connection_instant = None;
                event_sender
                    .send(MqttEvent::ConnectionStable {
                        connection_id: current_connection_id,
                    })
                    .await;
            }
        }

        // Check for too long since last connection event
        let elapsed = state.borrow().last_connection_event.elapsed();
        if elapsed > settings.connection_event_max_interval {
            #[cfg(feature = "defmt")]
            defmt::warn!("Mqtt server unresponsive");
            #[cfg(feature = "log")]
            log::warn!("Mqtt server unresponsive");
            return Err(Error::MqttServerUnresponsive);
        }

        // Poll with no delay while we have mqtt packets
        while client.poll(false).await? {}

        // If we have a pending action, try to perform it
        let pending_action = state.borrow_mut().pending_action.take();
        if let Some(action) = pending_action {
            try_action(
                current_connection_id,
                client,
                state,
                connection_settings,
                action,
                true,
            )
            .await?;
        }

        // Handle actions from receiver
        while let Ok(action) = action_receiver.try_receive() {
            try_action(
                current_connection_id,
                client,
                state,
                connection_settings,
                action,
                false,
            )
            .await?;
        }
    }
}

/// Run MQTT activities using the provided stack and settings.
/// This uses [Client], but via [embassy_sync::channel::Channel]s for use
/// from embassy.
///
/// It implements the following additional features compared to [Client]:
///
/// 1. When an error is encountered, the current [mountain_mqtt::packet_client::Connection] and [Client] are dropped, and a new connection/client made.
/// 2. [mountain_mqtt::packets::pingreq::Pingreq]s will automatically be sent to keep the connection alive, and if the server does not send acknowledgements for more than a timeout interval, the connection/client will be dropped and reconnected.
///
/// An [MqttEvent] is sent to `event_sender` whenever a relevant event occurs;
/// this will be either a connection, a disconnection, or receiving
/// an [ApplicationMessage]. When an [ApplicationMessage] is received, it is
/// converted into an `E` to be sent, allowing the use of application-specific
/// events.
///
/// Each time a new connection is made, it is assigned a [ConnectionId], and
/// each [MqttEvent] has an associated [ConnectionId] (including the events on
/// connecting and disconnecting).
///
/// While a connection is active, actions will be pulled from `action_receiver`,
/// and the [MqttOperations] trait will be used to allow each action to perform operations
/// on the active MQTT [Client]. The [MqttOperations] trait provides the current
/// [ConnectionId], so it is possible to tie actions to a specific connection.
/// So if an action is specific to a given connection and is called with a
/// different current [ConnectionId], it can just skip any operations it would
/// perform on the [Client] - this is useful for actions that subscribe to
/// topics in response to a new connection.
/// A similar approach is used for retrying operations - if an operation fails,
/// it will be called again on reconnection, with `is_retry` set to true, and
/// can then decide whether to retry operations on the client, or just skip.
///
/// This approach doesn't cover all possible sequences of operations, but
/// should cover a lot of common use cases, via something like the following
/// usage:
///
/// 1. Set up your network stack, define [Settings] and [ConnectionSettings], and create [embassy_sync::channel::Channel]s for events and actions.
/// 2. Make a new task to call this function, and start the task.
/// 3. Start feeding actions to the action channel, and receiving events on the event channel.
///
/// In most cases, you will want to respond to an [MqttEvent::Connected] event
/// by sending actions to subscribe to relevant topics (if any), including the
/// [ConnectionId] from the event in the action and checking this against the
/// current [ConnectionId] provided when [MqttOperations::perform] is called.
/// By specifying the id, we ensure that if the connection is closed before
/// the actions are handled, they will be ignored - each connection will get
/// its own set of subscription actions, to avoid performing extra subscriptions etc.
///
/// In many cases, you can send actions that result in publishing messages
/// without a [ConnectionId], and they can then publish to the [Client] ignoring
/// the current [ConnectionId].
///
/// This function uses type parameters to allow it to work with
/// application-specific types and data sizes:
///
/// - `A` for the action type used to perform operations on the MQTT server
/// - `E` for the event type produced from [ApplicationMessage]s received from the MQTT server.
/// - `P` for the maximum property size expected in sent and received MQTT packets.
/// - `B` for the size of buffers used for network receive and transmit, and for processing MQTT packets.
/// - `Q` for the size of the channels used for events and actions.
///
/// Generally you will want to call this function as an embassy task - since it
/// contains type parameters it can't be used directly, so you can just wrap it
/// in another function that specifies the type parameters directly, e.g. see the
/// example in the `embassy-example-rp2040w` crate, in the `example_mqtt_manager`
/// module.
///
pub async fn run<A, E, const P: usize, const B: usize, const Q: usize>(
    stack: Stack<'static>,
    connection_settings: ConnectionSettings<'static>,
    settings: Settings,
    event_sender: Sender<'static, NoopRawMutex, MqttEvent<E>, Q>,
    mut action_receiver: Receiver<'static, NoopRawMutex, A, Q>,
) -> !
where
    E: FromApplicationMessage<P> + Clone,
    A: MqttOperations + Clone,
{
    let mut rx_buffer = [0; B];
    let mut tx_buffer = [0; B];
    let mut mqtt_buffer = [0; B];

    let mut connection_index = 0u32;

    loop {
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

        socket.set_timeout(None);

        let remote_endpoint = (settings.address, settings.port);
        #[cfg(feature = "defmt")]
        defmt::info!("MQTT socket connecting to {:?}...", remote_endpoint);
        #[cfg(feature = "log")]
        log::info!("MQTT socket connecting to {:?}...", remote_endpoint);

        if let Err(e) = socket.connect(remote_endpoint).await {
            #[cfg(feature = "defmt")]
            defmt::warn!("MQTT socket connect error, will retry: {:?}", e);
            #[cfg(feature = "log")]
            log::warn!("MQTT socket connect error, will retry: {:?}", e);
            // Wait a while to try reconnecting
            Timer::after(settings.reconnection_delay).await;
            continue;
        }

        #[cfg(feature = "defmt")]
        defmt::info!("MQTT socket connected!");
        #[cfg(feature = "log")]
        log::info!("MQTT socket connected!");

        let connection = ConnectionEmbedded::new(socket);
        let delay = DelayEmbedded::new(Delay);
        let timeout_millis = settings.response_timeout.as_millis() as u32;

        let state: RefCell<State<A>> = RefCell::new(State::new());

        let connection_id = ConnectionId::new(connection_index);
        connection_index += 1;

        let event_handler = ChannelEventHandler {
            connection_id,
            event_sender: &event_sender,
            state: &state,
        };

        let mut client = ClientNoQueue::new(
            connection,
            &mut mqtt_buffer,
            delay,
            timeout_millis,
            event_handler,
        );

        if let Err(error) = handle_messages(
            connection_id,
            &mut client,
            &state,
            &connection_settings,
            &event_sender,
            &mut action_receiver,
            &settings,
        )
        .await
        {
            #[cfg(feature = "defmt")]
            defmt::warn!("MQTT handle_messages errored: {:?}", error);
            #[cfg(feature = "log")]
            log::warn!("MQTT handle_messages errored: {:?}", error);
            event_sender
                .send(MqttEvent::Disconnected {
                    connection_id,
                    error,
                })
                .await;
        }

        // Wait a while to try reconnecting
        Timer::after(settings.reconnection_delay).await;
    }
}
