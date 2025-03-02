use core::fmt::{Display, Formatter};

use heapless::Vec;

use crate::{
    client_state::{ClientState, ClientStateError, ClientStateNoQueue, ClientStateReceiveEvent},
    codec::write,
    data::{
        property::ConnectProperty, quality_of_service::QualityOfService,
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

/// [Client] error
#[derive(Debug, PartialEq)]
pub enum ClientError<E> {
    PacketWrite(PacketWriteError),
    PacketRead(PacketReadError),
    ClientState(ClientStateError),
    TimeoutOnResponsePacket,
    Disconnected(DisconnectReasonCode),
    MessageHandler(E),
    /// Client received an empty topic name when it has disabled topic aliases
    /// This indicates a server error, client should disconnect, it may send
    /// a Disconnect with [DisconnectReasonCode::TopicAliasInvalid], on the assumption
    /// that the packet also had some topic alias specified.
    EmptyTopicNameWithAliasesDisabled,
}

#[cfg(feature = "defmt")]
impl<E> defmt::Format for ClientError<E> {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::PacketWrite(e) => defmt::write!(f, "PacketWrite({})", e),
            Self::PacketRead(e) => defmt::write!(f, "PacketRead({})", e),
            Self::ClientState(e) => defmt::write!(f, "ClientState({})", e),
            Self::TimeoutOnResponsePacket => defmt::write!(f, "TimeoutOnResponsePacket"),
            Self::Disconnected(r) => defmt::write!(f, "Disconnected({})", r),
            Self::MessageHandler(_) => defmt::write!(f, "MessageHandler"),
            Self::EmptyTopicNameWithAliasesDisabled => {
                defmt::write!(f, "EmptyTopicNameWithAliasesDisabled")
            }
        }
    }
}

impl<E> From<ClientStateError> for ClientError<E> {
    fn from(value: ClientStateError) -> Self {
        ClientError::ClientState(value)
    }
}

impl<E> From<PacketWriteError> for ClientError<E> {
    fn from(value: PacketWriteError) -> Self {
        ClientError::PacketWrite(value)
    }
}

impl<E> From<PacketReadError> for ClientError<E> {
    fn from(value: PacketReadError) -> Self {
        ClientError::PacketRead(value)
    }
}

impl<E> Display for ClientError<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PacketWrite(e) => write!(f, "PacketWrite({})", e),
            Self::PacketRead(e) => write!(f, "PacketRead({})", e),
            Self::ClientState(e) => write!(f, "ClientState({})", e),
            Self::TimeoutOnResponsePacket => write!(f, "TimeoutOnResponsePacket"),
            Self::Disconnected(e) => write!(f, "Disconnected({})", e),
            Self::MessageHandler(_) => write!(f, "MessageHandlerError"),
            Self::EmptyTopicNameWithAliasesDisabled => write!(f, "EmptyTopicWithAliasesDisabled"),
        }
    }
}

/// A simple client interface for connecting to an MQTT server
#[allow(async_fn_in_trait)]
pub trait Client<'a, E> {
    /// Connect to server
    async fn connect(&mut self, settings: ConnectionSettings) -> Result<(), ClientError<E>>;

    /// Disconnect from server
    async fn disconnect(&mut self) -> Result<(), ClientError<E>>;

    /// Send a ping message to broker
    async fn send_ping(&mut self) -> Result<(), ClientError<E>>;

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
    async fn poll(&mut self, wait: bool) -> Result<bool, ClientError<E>>;

    /// Subscribe to a topic
    async fn subscribe<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError<E>>;

    /// Unsubscribe from a topic
    async fn unsubscribe<'b>(&'b mut self, topic_name: &'b str) -> Result<(), ClientError<E>>;

    /// Publish a message with given payload to a given topic
    async fn publish<'b>(
        &'b mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError<E>>;
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

pub struct ClientNoQueue<'a, C, D, F, E, const P: usize>
where
    C: Connection,
    D: Delay,
    F: Fn(ClientReceivedEvent<P>) -> Result<(), E>,
{
    packet_client: PacketClient<'a, C>,
    client_state: ClientStateNoQueue,
    delay: D,
    timeout_millis: u32,
    event_handler: F,
}

impl<'a, C, D, F, E, const P: usize> ClientNoQueue<'a, C, D, F, E, P>
where
    C: Connection,
    D: Delay,
    F: Fn(ClientReceivedEvent<P>) -> Result<(), E>,
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

    async fn wait_for_responses(&mut self, timeout_millis: u32) -> Result<(), ClientError<E>> {
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

    async fn send_wait_for_responses<PW>(&mut self, packet: PW) -> Result<(), ClientError<E>>
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

    async fn send<PW>(&mut self, packet: PW) -> Result<(), ClientError<E>>
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

impl<'a, C, D, F, E, const P: usize> Client<'a, E> for ClientNoQueue<'a, C, D, F, E, P>
where
    C: Connection,
    D: Delay,
    F: Fn(ClientReceivedEvent<P>) -> Result<(), E>,
{
    async fn connect(&mut self, settings: ConnectionSettings<'_>) -> Result<(), ClientError<E>> {
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

    async fn disconnect(&mut self) -> Result<(), ClientError<E>> {
        let packet = self.client_state.disconnect()?;
        self.send(packet).await
    }

    async fn publish<'b>(
        &'b mut self,
        topic_name: &'b str,
        message: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError<E>> {
        let packet = self
            .client_state
            .publish(topic_name, message, qos, retain)?;
        self.send_wait_for_responses(packet).await
    }

    async fn subscribe<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError<E>> {
        let packet = self.client_state.subscribe(topic_name, maximum_qos)?;
        self.send_wait_for_responses(packet).await
    }

    async fn unsubscribe<'b>(&'b mut self, topic_name: &'b str) -> Result<(), ClientError<E>> {
        let packet = self.client_state.unsubscribe(topic_name)?;
        self.send_wait_for_responses(packet).await
    }

    async fn send_ping(&mut self) -> Result<(), ClientError<E>> {
        let packet = self.client_state.send_ping()?;
        self.send(packet).await
    }

    async fn poll(&mut self, wait: bool) -> Result<bool, ClientError<E>> {
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
                        (self.event_handler)(ClientReceivedEvent::Ack)
                            .map_err(|e| ClientError::MessageHandler(e))?;
                        None
                    }

                    ClientStateReceiveEvent::Publish { publish } => {
                        if publish.topic_name().is_empty() {
                            return Err(ClientError::EmptyTopicNameWithAliasesDisabled);
                        }
                        (self.event_handler)(publish.into())
                            .map_err(|e| ClientError::MessageHandler(e))?;
                        None
                    }

                    ClientStateReceiveEvent::PublishAndPuback { publish, puback } => {
                        if publish.topic_name().is_empty() {
                            return Err(ClientError::EmptyTopicNameWithAliasesDisabled);
                        }
                        (self.event_handler)(publish.into())
                            .map_err(|e| ClientError::MessageHandler(e))?;
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
                        )
                        .map_err(|e| ClientError::MessageHandler(e))?;
                        None
                    }

                    ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers => {
                        (self.event_handler)(
                            ClientReceivedEvent::PublishedMessageHadNoMatchingSubscribers,
                        )
                        .map_err(|e| ClientError::MessageHandler(e))?;
                        None
                    }

                    ClientStateReceiveEvent::NoSubscriptionExisted => {
                        (self.event_handler)(ClientReceivedEvent::NoSubscriptionExisted)
                            .map_err(|e| ClientError::MessageHandler(e))?;
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
}
