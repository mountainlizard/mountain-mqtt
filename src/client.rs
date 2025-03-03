use core::{
    fmt::{Display, Formatter},
    str::Utf8Error,
};

use heapless::Vec;

use crate::{
    client_state::{ClientState, ClientStateError, ClientStateNoQueue, ClientStateReceiveEvent},
    codec::write,
    data::{
        property::{ConnectProperty, PublishProperty},
        quality_of_service::QualityOfService,
        reason_code::DisconnectReasonCode,
    },
    error::{PacketReadError, PacketWriteError},
    packet_client::{Connection, PacketClient},
    packets::{
        connect::Connect,
        packet::{Packet, KEEP_ALIVE_DEFAULT},
        packet_generic::PacketGeneric,
        publish::{ApplicationMessage, Publish},
    },
};

/// Errors produced when a [ClientNoQueue] event handler cannot handle
/// a [ClientReceivedEvent]. These errors propagate to the user of the
/// client, and so are likely to cause the client to be disconnected.
/// Alternatively, an event handler can use another method to propagate
/// an error if it does not wish for the client to be disconnected, for
/// example logging a warning, or using another target for the error,
/// for example if an event handler uses a channel to send valid events
/// onwards, it could also have a channel for errors.
#[derive(Debug, PartialEq)]
pub enum EventHandlerError {
    /// Application Message payload contained invalid utf8 data, when
    /// a utf8 string was expected
    Utf8(Utf8Error),

    /// The topic of an application message is not expected, for example
    /// it doesn't match our expected subscriptions
    UnexpectedApplicationMessageTopic,

    /// The contents of an application message are invalid for the handler,
    /// and can't be parsed.
    /// For example if a json string is expected, and invalid json is received,
    /// or if the json received does not match the expected schema.
    InvalidApplicationMessage,

    /// The contents of an application message can be parsed, but are unexpected,
    /// e.g. if they are received out of sequence
    UnexpectedApplicationMessage,

    /// A valid, expected event was received, but could not be
    /// handled due to an overflow. For example if the messages are sent onwards
    /// to a channel, and that channel is at capacity.
    Overflow,

    /// The corresponding [ClientReceivedEvent] was received, and is an error
    /// for this event handler
    SubscriptionGrantedBelowMaximumQos {
        granted_qos: QualityOfService,
        maximum_qos: QualityOfService,
    },

    /// The corresponding [ClientReceivedEvent] was received, and is an error
    /// for this event handler
    PublishedMessageHadNoMatchingSubscribers,

    /// The corresponding [ClientReceivedEvent] was received, and is an error
    /// for this event handler
    NoSubscriptionExisted,
}
#[cfg(feature = "defmt")]
impl defmt::Format for EventHandlerError {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::Utf8(_) => defmt::write!(f, "Utf8"),
            Self::UnexpectedApplicationMessageTopic => {
                defmt::write!(f, "UnexpectedApplicationMessageTopic")
            }
            Self::InvalidApplicationMessage => defmt::write!(f, "InvalidApplicationMessage"),
            Self::UnexpectedApplicationMessage => defmt::write!(f, "UnexpectedApplicationMessage"),
            Self::Overflow => defmt::write!(f, "Overflow"),
            Self::SubscriptionGrantedBelowMaximumQos {
                granted_qos,
                maximum_qos,
            } => defmt::write!(
                f,
                "SubscriptionGrantedBelowMaximumQos(granted_qos: {}, maximum_qos: {})",
                granted_qos,
                maximum_qos
            ),
            Self::PublishedMessageHadNoMatchingSubscribers => {
                defmt::write!(f, "PublishedMessageHadNoMatchingSubscribers")
            }
            Self::NoSubscriptionExisted => defmt::write!(f, "NoSubscriptionExisted"),
        }
    }
}

impl From<Utf8Error> for EventHandlerError {
    fn from(value: Utf8Error) -> Self {
        Self::Utf8(value)
    }
}

impl Display for EventHandlerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Utf8(e) => write!(f, "Utf8({})", e),
            Self::UnexpectedApplicationMessageTopic => {
                write!(f, "UnexpectedApplicationMessageTopic")
            }
            Self::InvalidApplicationMessage => write!(f, "InvalidApplicationMessage"),
            Self::UnexpectedApplicationMessage => write!(f, "UnexpectedApplicationMessage"),
            Self::Overflow => write!(f, "Overflow"),
            Self::SubscriptionGrantedBelowMaximumQos {
                granted_qos,
                maximum_qos,
            } => write!(
                f,
                "SubscriptionGrantedBelowMaximumQos(granted_qos: {}, maximum_qos: {})",
                granted_qos, maximum_qos
            ),
            Self::PublishedMessageHadNoMatchingSubscribers => {
                write!(f, "PublishedMessageHadNoMatchingSubscribers")
            }
            Self::NoSubscriptionExisted => write!(f, "NoSubscriptionExisted"),
        }
    }
}

/// [Client] error
#[derive(Debug, PartialEq)]
pub enum ClientError {
    PacketWrite(PacketWriteError),
    PacketRead(PacketReadError),
    ClientState(ClientStateError),
    TimeoutOnResponsePacket,
    Disconnected(DisconnectReasonCode),
    EventHandler(EventHandlerError),
    /// Client received an empty topic name when it has disabled topic aliases
    /// This indicates a server error, client should disconnect, it may send
    /// a Disconnect with [DisconnectReasonCode::TopicAliasInvalid], on the assumption
    /// that the packet also had some topic alias specified.
    EmptyTopicNameWithAliasesDisabled,
}

#[cfg(feature = "defmt")]
impl defmt::Format for ClientError {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::PacketWrite(e) => defmt::write!(f, "PacketWrite({})", e),
            Self::PacketRead(e) => defmt::write!(f, "PacketRead({})", e),
            Self::ClientState(e) => defmt::write!(f, "ClientState({})", e),
            Self::TimeoutOnResponsePacket => defmt::write!(f, "TimeoutOnResponsePacket"),
            Self::Disconnected(r) => defmt::write!(f, "Disconnected({})", r),
            Self::EventHandler(e) => defmt::write!(f, "EventHandler({})", e),
            Self::EmptyTopicNameWithAliasesDisabled => {
                defmt::write!(f, "EmptyTopicNameWithAliasesDisabled")
            }
        }
    }
}

impl From<ClientStateError> for ClientError {
    fn from(value: ClientStateError) -> Self {
        ClientError::ClientState(value)
    }
}

impl From<PacketWriteError> for ClientError {
    fn from(value: PacketWriteError) -> Self {
        ClientError::PacketWrite(value)
    }
}

impl From<PacketReadError> for ClientError {
    fn from(value: PacketReadError) -> Self {
        ClientError::PacketRead(value)
    }
}

impl From<EventHandlerError> for ClientError {
    fn from(value: EventHandlerError) -> Self {
        ClientError::EventHandler(value)
    }
}

impl Display for ClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PacketWrite(e) => write!(f, "PacketWrite({})", e),
            Self::PacketRead(e) => write!(f, "PacketRead({})", e),
            Self::ClientState(e) => write!(f, "ClientState({})", e),
            Self::TimeoutOnResponsePacket => write!(f, "TimeoutOnResponsePacket"),
            Self::Disconnected(e) => write!(f, "Disconnected({})", e),
            Self::EventHandler(e) => write!(f, "EventHandler({})", e),
            Self::EmptyTopicNameWithAliasesDisabled => write!(f, "EmptyTopicWithAliasesDisabled"),
        }
    }
}

/// A simple client interface for connecting to an MQTT server
#[allow(async_fn_in_trait)]
pub trait Client<'a> {
    /// Connect to server
    async fn connect(&mut self, settings: &ConnectionSettings) -> Result<(), ClientError>;

    /// Disconnect from server
    async fn disconnect(&mut self) -> Result<(), ClientError>;

    /// Send a ping message to broker
    async fn send_ping(&mut self) -> Result<(), ClientError>;

    /// Poll for and handle at most one event
    /// This updates the state of the client, and calls the event_handler if
    /// a message is received
    /// If wait is true, this will wait until an event is received, or the comms
    /// are disconnected. Otherwise an event will only be waited for if at least one
    /// byte of data is already available, indicating a packet should be available
    /// soon.
    /// On success, returns true if an event was handled, false if none was received
    /// Errors indicate an invalid packet was received, message_target errored,
    /// the received packet was unexpected based on our state, or the comms are
    /// disconnected.
    async fn poll(&mut self, wait: bool) -> Result<bool, ClientError>;

    /// Subscribe to a topic
    async fn subscribe<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError>;

    /// Unsubscribe from a topic
    async fn unsubscribe<'b>(&'b mut self, topic_name: &'b str) -> Result<(), ClientError>;

    /// Publish a message with given payload to a given topic, with no properties
    async fn publish<'b>(
        &'b mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError> {
        self.publish_with_properties::<0>(topic_name, payload, qos, retain, Vec::new())
            .await
    }

    /// Publish a message with given payload to a given topic, with properties
    async fn publish_with_properties<'b, const P: usize>(
        &'b mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
        properties: Vec<PublishProperty<'b>, P>,
    ) -> Result<(), ClientError>;

    /// Perform an action (this replicates the functionality of
    /// [Client::subscribe], [Client::unsubscribe] and [Client::publish])
    /// but using an enum to represent the action.
    async fn perform<'b, const P: usize>(
        &'b mut self,
        action: ClientAction<'b, P>,
    ) -> Result<(), ClientError>;
}

pub enum ClientAction<'a, const P: usize> {
    Subscribe {
        topic_name: &'a str,
        maximum_qos: QualityOfService,
    },
    Unsubscribe {
        topic_name: &'a str,
    },
    Publish {
        topic_name: &'a str,
        payload: &'a [u8],
        qos: QualityOfService,
        retain: bool,
    },
    PublishWithProperties {
        topic_name: &'a str,
        payload: &'a [u8],
        qos: QualityOfService,
        retain: bool,
        properties: Vec<PublishProperty<'a>, P>,
    },
}

#[allow(async_fn_in_trait)]
pub trait Delay {
    /// Pauses execution for at minimum `us` microseconds. Pause can be longer
    /// if the implementation requires it due to precision/timing issues.
    async fn delay_us(&mut self, us: u32);
}

pub struct ConnectionSettings<'a> {
    keep_alive: u16,
    username: Option<&'a str>,
    password: Option<&'a [u8]>,
    client_id: &'a str,
}

impl<'a> ConnectionSettings<'a> {
    pub fn unauthenticated(client_id: &'a str) -> ConnectionSettings<'a> {
        Self {
            keep_alive: KEEP_ALIVE_DEFAULT,
            username: None,
            password: None,
            client_id,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ClientReceivedEvent<'a, const P: usize> {
    /// Client received an application message published to a subscribed topic
    ApplicationMessage(ApplicationMessage<'a, P>),

    /// Client received an acknowledgement/response for a previous message sent
    /// to the server (e.g. Connack, Puback, Suback, Unsuback, Pingresp)
    /// This can be used to track whether the client is still connected to the
    /// server - in particular, there will be an Ack event per ping response.
    /// An approach is to call [Client::send_ping] at least every T seconds,
    /// and consider the client to be still connected if there has been an
    /// Ack within the last T+N seconds, for some multiple N depending on
    /// how long a latency/interruption can be tolerated. To tolerate loss
    /// of ping request/response packets, N should be increased so that T+N
    /// is a multiple of T.
    Ack,

    /// A subscription was granted but was at lower qos than the maximum requested
    /// This may or may not require action depending on client requirements -
    /// it means that the given subscription will receive published messages at
    /// only the granted qos - if the requested maximum qos was absolutely required
    /// then the client could respond by showing an error to the user stating the
    /// server is incompatible, or possibly trying to unsubscribe and resubscribe,
    /// assuming this is expected to make any difference with the server(s) in use.
    SubscriptionGrantedBelowMaximumQos {
        granted_qos: QualityOfService,
        maximum_qos: QualityOfService,
    },

    /// A published message was received at the server, but had no matching subscribers and
    /// so did not reach any receivers
    /// This may or may not require action depending on client requirements / expectations
    /// E.g. if it was expected there would be subscribers, the client could try resending
    /// the message later
    PublishedMessageHadNoMatchingSubscribers,

    // Server processed an unsubscribe request, but no such subscription existed on the server,
    // so nothing changed.
    /// This may or may not require action depending on client requirements / expectations
    /// E.g. if it was expected there would be a subscription, the client could produce
    /// an error, and the user of the client might try reconnecting to the server to set
    /// up subscriptions again.
    NoSubscriptionExisted,
}

impl<'a, const P: usize> From<Publish<'a, P>> for ClientReceivedEvent<'a, P> {
    fn from(value: Publish<'a, P>) -> Self {
        Self::ApplicationMessage(value.into())
    }
}

pub struct ClientNoQueue<'a, C, D, F, const P: usize>
where
    C: Connection,
    D: Delay,
    F: Fn(ClientReceivedEvent<P>) -> Result<(), EventHandlerError>,
{
    packet_client: PacketClient<'a, C>,
    client_state: ClientStateNoQueue,
    delay: D,
    timeout_millis: u32,
    event_handler: F,
}

impl<'a, C, D, F, const P: usize> ClientNoQueue<'a, C, D, F, P>
where
    C: Connection,
    D: Delay,
    F: Fn(ClientReceivedEvent<P>) -> Result<(), EventHandlerError>,
{
    pub fn new(
        connection: C,
        buf: &'a mut [u8],
        delay: D,
        timeout_millis: u32,
        event_handler: F,
    ) -> Self {
        let packet_client = PacketClient::new(connection, buf);
        let client_state = ClientStateNoQueue::default();
        Self {
            packet_client,
            client_state,
            delay,
            timeout_millis,
            event_handler,
        }
    }

    async fn wait_for_responses(&mut self, timeout_millis: u32) -> Result<(), ClientError> {
        let mut elapsed = 0;
        let mut waiting = self.client_state.waiting_for_responses();
        while waiting && elapsed <= timeout_millis {
            self.poll(false).await?;
            waiting = self.client_state.waiting_for_responses();
            elapsed += 1;
            self.delay.delay_us(1000).await;
        }

        if waiting {
            Err(ClientError::TimeoutOnResponsePacket)
        } else {
            Ok(())
        }
    }

    async fn send_wait_for_responses<PW>(&mut self, packet: PW) -> Result<(), ClientError>
    where
        PW: Packet + write::Write,
    {
        match self.packet_client.send(packet).await {
            Ok(()) => {
                self.wait_for_responses(self.timeout_millis).await?;
                Ok(())
            }
            Err(e) => {
                self.client_state.error();
                Err(e.into())
            }
        }
    }

    async fn send<PW>(&mut self, packet: PW) -> Result<(), ClientError>
    where
        PW: Packet + write::Write,
    {
        let r = self.packet_client.send(packet).await;
        if r.is_err() {
            self.client_state.error();
        }
        r?;
        Ok(())
    }
}

impl<'a, C, D, F, const P: usize> Client<'a> for ClientNoQueue<'a, C, D, F, P>
where
    C: Connection,
    D: Delay,
    F: Fn(ClientReceivedEvent<P>) -> Result<(), EventHandlerError>,
{
    async fn connect(&mut self, settings: &ConnectionSettings<'_>) -> Result<(), ClientError> {
        let mut properties = Vec::new();
        // By setting maximum topic alias to 0, we prevent the server
        // trying to use aliases, which we don't support. They are optional
        // and only provide for reduced packet size, but would require storing
        // topic names from the server for the length of the connection,
        // which might be awkward without alloc.
        properties
            .push(ConnectProperty::TopicAliasMaximum(0.into()))
            .unwrap();
        let packet: Connect<'_, 1> = Connect::new(
            settings.keep_alive,
            settings.username,
            settings.password,
            settings.client_id,
            true,
            None,
            properties,
        );
        self.client_state.connect(&packet)?;
        self.send_wait_for_responses(packet).await
    }

    async fn disconnect(&mut self) -> Result<(), ClientError> {
        let packet = self.client_state.disconnect()?;
        self.send(packet).await
    }

    async fn publish_with_properties<'b, const PP: usize>(
        &'b mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
        properties: Vec<PublishProperty<'b>, PP>,
    ) -> Result<(), ClientError> {
        let packet = self
            .client_state
            .publish_with_properties(topic_name, payload, qos, retain, properties)?;
        self.send_wait_for_responses(packet).await
    }

    async fn subscribe<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError> {
        let packet = self.client_state.subscribe(topic_name, maximum_qos)?;
        self.send_wait_for_responses(packet).await
    }

    async fn unsubscribe<'b>(&'b mut self, topic_name: &'b str) -> Result<(), ClientError> {
        let packet = self.client_state.unsubscribe(topic_name)?;
        self.send_wait_for_responses(packet).await
    }

    async fn send_ping(&mut self) -> Result<(), ClientError> {
        let packet = self.client_state.send_ping()?;
        self.send(packet).await
    }

    async fn poll(&mut self, wait: bool) -> Result<bool, ClientError> {
        // We need to wrap up like this so we can drop the mutable reference to
        // self.packet_client needed to receive data - this reference needs to live
        // as long as the returned data from the client, so we need to drop everything
        // but the packet we need to send, in order to be able to mutably borrow
        // packet_client again to actually do the send.
        let to_send = {
            let packet: Option<PacketGeneric<'_, P, 0>> = if wait {
                Some(self.packet_client.receive().await?)
            } else {
                self.packet_client.receive_if_ready().await?
            };

            if let Some(packet) = packet {
                let event = self.client_state.receive(packet)?;

                match event {
                    ClientStateReceiveEvent::Ack => {
                        (self.event_handler)(ClientReceivedEvent::Ack)?;
                        None
                    }

                    ClientStateReceiveEvent::Publish { publish } => {
                        if publish.topic_name().is_empty() {
                            return Err(ClientError::EmptyTopicNameWithAliasesDisabled);
                        }
                        (self.event_handler)(publish.into())?;
                        None
                    }

                    ClientStateReceiveEvent::PublishAndPuback { publish, puback } => {
                        if publish.topic_name().is_empty() {
                            return Err(ClientError::EmptyTopicNameWithAliasesDisabled);
                        }
                        (self.event_handler)(publish.into())?;
                        Some(puback)
                    }

                    ClientStateReceiveEvent::SubscriptionGrantedBelowMaximumQos {
                        granted_qos,
                        maximum_qos,
                    } => {
                        (self.event_handler)(
                            ClientReceivedEvent::SubscriptionGrantedBelowMaximumQos {
                                granted_qos,
                                maximum_qos,
                            },
                        )?;
                        None
                    }

                    ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers => {
                        (self.event_handler)(
                            ClientReceivedEvent::PublishedMessageHadNoMatchingSubscribers,
                        )?;
                        None
                    }

                    ClientStateReceiveEvent::NoSubscriptionExisted => {
                        (self.event_handler)(ClientReceivedEvent::NoSubscriptionExisted)?;
                        None
                    }

                    ClientStateReceiveEvent::Disconnect { disconnect } => {
                        return Err(ClientError::Disconnected(*disconnect.reason_code()));
                    }
                }
            } else {
                return Ok(false);
            }
        };

        // Send any resulting packet, no need to wait for responses
        if let Some(packet) = to_send {
            self.send(packet).await?;
        }

        Ok(true)
    }

    async fn perform<'b, const PP: usize>(
        &'b mut self,
        action: ClientAction<'b, PP>,
    ) -> Result<(), ClientError> {
        match action {
            ClientAction::Subscribe {
                topic_name,
                maximum_qos,
            } => self.subscribe(topic_name, maximum_qos).await,
            ClientAction::Unsubscribe { topic_name } => self.unsubscribe(topic_name).await,
            ClientAction::Publish {
                topic_name,
                payload,
                qos,
                retain,
            } => self.publish(topic_name, payload, qos, retain).await,
            ClientAction::PublishWithProperties {
                topic_name,
                payload,
                qos,
                retain,
                properties,
            } => {
                self.publish_with_properties(topic_name, payload, qos, retain, properties)
                    .await
            }
        }
    }
}
