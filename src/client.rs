use core::fmt;
use core::fmt::Write;

use heapless::{String, Vec};

use crate::{
    data::{
        packet_identifier::{PacketIdentifier, PublishPacketIdentifier},
        quality_of_service::QualityOfService,
        reason_code::{
            ConnectReasonCode, PublishReasonCode, SubscribeReasonCode, UnsubscribeReasonCode,
        },
    },
    error::{PacketReadError, PacketWriteError},
    packet_client::{ConnectionReady, PacketClient},
    packets::{
        connect::Connect,
        disconnect::Disconnect,
        packet_generic::PacketGeneric,
        pingreq::Pingreq,
        publish::Publish,
        subscribe::{Subscribe, SubscriptionRequest},
        unsubscribe::Unsubscribe,
    },
};

/// [Client] error
#[derive(Debug, PartialEq)]
pub enum ClientError {
    Send(PacketWriteError),
    Receive(PacketReadError),
    TimeoutOnResponsePacket,
    NotIdle,
    Idle,
    UnsupportedFeature,
    QoS1MessagePending,
    NotConnected,
    SubscriptionPending,
    UnsubscriptionPending,
    UnexpectedPuback,
    UnexpectedPubackPacketIdentifier,
    UnexpectedSuback,
    UnexpectedSubackPacketIdentifier,
    UnexpectedUnsuback,
    UnexpectedUnsubackPacketIdentifier,
    UnexpectedPingresp,
    Disconnect,
    ServerOnlyMessageReceived,
    Format(core::fmt::Error),
    Connect(ConnectReasonCode),
    Subscribe(SubscribeReasonCode),
    Publish(PublishReasonCode),
    Unsubscribe(UnsubscribeReasonCode),
}

impl From<PacketWriteError> for ClientError {
    fn from(value: PacketWriteError) -> Self {
        ClientError::Send(value)
    }
}

impl From<PacketReadError> for ClientError {
    fn from(value: PacketReadError) -> Self {
        ClientError::Receive(value)
    }
}

impl From<core::fmt::Error> for ClientError {
    fn from(value: core::fmt::Error) -> Self {
        Self::Format(value)
    }
}

/// A simple client interface for connecting to an MQTT server
#[allow(async_fn_in_trait)]
pub trait Client {
    /// Connect to server
    async fn connect<const PROPERTIES_N: usize>(
        &mut self,
        connect: Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientError>;

    /// Disconnect from server
    async fn disconnect(&mut self) -> Result<(), ClientError>;

    /// Send a ping message to broker
    async fn send_ping(&mut self) -> Result<(), ClientError>;

    /// Poll for and handle at most one event
    /// This updates the state of the manager, and calls the message_handler if
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

    /// Subscribe to a topic by name
    async fn subscribe_to_topic<'b>(&'b mut self, topic_name: &'b str) -> Result<(), ClientError>;

    /// Subscribe to a topic, providing the topic name as format arguments
    async fn subscribe_to_topic_fmt<'b, const L: usize>(
        &'b mut self,
        topic_name: fmt::Arguments<'b>,
    ) -> Result<(), ClientError>;

    /// Unsubscribe from a topic by name.
    async fn unsubscribe_from_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
    ) -> Result<(), ClientError>;

    /// Send a message to a given topic, on the connected broker.
    async fn send_message<'b>(
        &'b mut self,
        topic_name: &'b str,
        message: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError>;
}

#[derive(PartialEq)]
pub enum ConnectionState {
    Idle,
    Connecting,
    Connected,
}

#[allow(async_fn_in_trait)]
pub trait Delay {
    /// Pauses execution for at minimum `us` microseconds. Pause can be longer
    /// if the implementation requires it due to precision/timing issues.
    async fn delay_us(&mut self, us: u32);
}

pub struct ClientNoQueue<'a, C, D, F>
where
    C: ConnectionReady,
    D: Delay,
    F: Fn(&str, &[u8]) -> Result<(), ClientError>,
{
    packet_client: PacketClient<'a, C>,
    connection_state: ConnectionState,
    pending_puback_identifier: Option<PacketIdentifier>,
    pending_suback_identifier: Option<PacketIdentifier>,
    pending_unsuback_identifier: Option<PacketIdentifier>,
    pending_ping_count: u16,
    delay: D,
    timeout_millis: u32,
    message_handler: F,
}

impl<'a, C, D, F> ClientNoQueue<'a, C, D, F>
where
    C: ConnectionReady,
    D: Delay,
    F: Fn(&str, &[u8]) -> Result<(), ClientError>,
{
    const PUBLISH_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(1);
    const SUBSCRIBE_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(2);
    const UNSUBSCRIBE_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(3);

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        connection: C,
        buf: &'a mut [u8],
        delay: D,
        timeout_millis: u32,
        message_handler: F,
    ) -> Self {
        let packet_client = PacketClient::new(connection, buf);
        Self {
            packet_client,
            connection_state: ConnectionState::Idle,
            pending_puback_identifier: None,
            pending_suback_identifier: None,
            pending_unsuback_identifier: None,
            pending_ping_count: 0,
            delay,
            timeout_millis,
            message_handler,
        }
    }

    fn any_pending(&self) -> bool {
        self.pending_puback_identifier.is_some()
            || self.pending_suback_identifier.is_some()
            || self.pending_unsuback_identifier.is_some()
    }

    fn waiting_for_responses(&self) -> bool {
        self.connection_state == ConnectionState::Connecting
            || (self.connection_state == ConnectionState::Connected && self.any_pending())
    }

    // Wait while the manager is expecting a response
    // Call this after any function that requires a response - connect_to_broker,
    // subscribe_to_topic, unsubscribe_from_topic, send_message with QoS1
    async fn wait_for_responses(&mut self, timeout_millis: u32) -> Result<(), ClientError> {
        let mut elapsed = 0;
        let mut waiting = true;
        while waiting && elapsed <= timeout_millis {
            self.poll(false).await?;
            waiting = self.waiting_for_responses();
            elapsed += 1;
            self.delay.delay_us(1000).await;
        }

        if waiting {
            Err(ClientError::TimeoutOnResponsePacket)
        } else {
            Ok(())
        }
    }
}

impl<C, D, F> Client for ClientNoQueue<'_, C, D, F>
where
    C: ConnectionReady,
    D: Delay,
    F: Fn(&str, &[u8]) -> Result<(), ClientError>,
{
    async fn connect<const PROPERTIES_N: usize>(
        &mut self,
        connect: Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientError> {
        if self.connection_state == ConnectionState::Idle {
            self.packet_client.send(connect).await?;
            self.connection_state = ConnectionState::Connecting;
            self.wait_for_responses(self.timeout_millis).await?;
            Ok(())
        } else {
            Err(ClientError::NotIdle)
        }
    }

    async fn disconnect(&mut self) -> Result<(), ClientError> {
        if self.connection_state != ConnectionState::Idle {
            self.packet_client.send(Disconnect::default()).await?;
            self.connection_state = ConnectionState::Idle;
            Ok(())
        } else {
            Err(ClientError::Idle)
        }
    }

    async fn send_message<'b>(
        &'b mut self,
        topic_name: &'b str,
        message: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError> {
        if self.connection_state == ConnectionState::Connected {
            let publish_packet_identifier = match qos {
                QualityOfService::QoS0 => Ok(PublishPacketIdentifier::None),
                QualityOfService::QoS1 if self.pending_puback_identifier.is_some() => {
                    Err(ClientError::QoS1MessagePending)
                }
                QualityOfService::QoS1 => Ok(PublishPacketIdentifier::Qos1(
                    Self::PUBLISH_PACKET_IDENTIFIER,
                )),
                QualityOfService::QoS2 => Err(ClientError::UnsupportedFeature),
            }?;

            let publish: Publish<'_, 0> = Publish::new(
                false,
                retain,
                topic_name,
                publish_packet_identifier,
                message,
                Vec::new(),
            );

            self.packet_client.send(publish).await?;

            if qos == QualityOfService::QoS1 {
                self.pending_puback_identifier = Some(Self::PUBLISH_PACKET_IDENTIFIER);
                self.wait_for_responses(self.timeout_millis).await?;
            }

            Ok(())
        } else {
            Err(ClientError::NotConnected)
        }
    }

    async fn subscribe_to_topic<'b>(&'b mut self, topic_name: &'b str) -> Result<(), ClientError> {
        if self.connection_state == ConnectionState::Connected {
            if self.pending_suback_identifier.is_some() {
                Err(ClientError::SubscriptionPending)
            } else {
                let first_request = SubscriptionRequest::new(topic_name, QualityOfService::QoS0);
                let subscribe: Subscribe<'_, 0, 0> = Subscribe::new(
                    Self::SUBSCRIBE_PACKET_IDENTIFIER,
                    first_request,
                    Vec::new(),
                    Vec::new(),
                );

                self.packet_client.send(subscribe).await?;

                self.pending_suback_identifier = Some(Self::SUBSCRIBE_PACKET_IDENTIFIER);
                self.wait_for_responses(self.timeout_millis).await?;

                Ok(())
            }
        } else {
            Err(ClientError::NotConnected)
        }
    }

    async fn subscribe_to_topic_fmt<'b, const L: usize>(
        &'b mut self,
        topic_name: fmt::Arguments<'b>,
    ) -> Result<(), ClientError> {
        let mut topic_string = String::<L>::new();
        topic_string.write_fmt(topic_name)?;
        self.subscribe_to_topic(topic_string.as_str()).await
    }

    async fn unsubscribe_from_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
    ) -> Result<(), ClientError> {
        if self.connection_state == ConnectionState::Connected {
            if self.pending_unsuback_identifier.is_some() {
                Err(ClientError::UnsubscriptionPending)
            } else {
                let unsubscribe: Unsubscribe<'_, 0, 0> = Unsubscribe::new(
                    Self::UNSUBSCRIBE_PACKET_IDENTIFIER,
                    topic_name,
                    Vec::new(),
                    Vec::new(),
                );

                self.packet_client.send(unsubscribe).await?;

                self.pending_unsuback_identifier = Some(Self::UNSUBSCRIBE_PACKET_IDENTIFIER);
                self.wait_for_responses(self.timeout_millis).await?;
                Ok(())
            }
        } else {
            Err(ClientError::NotConnected)
        }
    }

    async fn send_ping(&mut self) -> Result<(), ClientError> {
        if self.connection_state == ConnectionState::Connected {
            self.packet_client.send(Pingreq::default()).await?;

            self.pending_ping_count += 1;
            Ok(())
        } else {
            Err(ClientError::NotConnected)
        }
    }

    async fn poll(&mut self, wait: bool) -> Result<bool, ClientError> {
        let event: Option<PacketGeneric<'_, 16, 16>> = if wait {
            Some(self.packet_client.receive().await?)
        } else {
            self.packet_client.receive_if_ready().await?
        };

        if let Some(packet) = event {
            match packet {
                PacketGeneric::Publish(publish) => {
                    (self.message_handler)(publish.topic_name(), publish.payload())?;
                    Ok(true)
                }
                PacketGeneric::Connack(connack) => match connack.reason_code() {
                    ConnectReasonCode::Success => {
                        self.connection_state = ConnectionState::Connected;
                        Ok(true)
                    }
                    reason_code => Err(ClientError::Connect(*reason_code)),
                },
                PacketGeneric::Puback(puback) => {
                    let reason_code = puback.reason_code();
                    // TODO: Should we do anything about NoMatchingSubscribers?
                    if reason_code.is_error() {
                        return Err(ClientError::Publish(*reason_code));
                    }

                    let ack_packet_identifier = puback.packet_identifier();
                    match &self.pending_puback_identifier {
                        Some(expected_packet_identifier) => {
                            if expected_packet_identifier == ack_packet_identifier {
                                self.pending_puback_identifier = None;
                                Ok(true)
                            } else {
                                Err(ClientError::UnexpectedPubackPacketIdentifier)
                            }
                        }
                        None => Err(ClientError::UnexpectedPuback),
                    }
                }
                PacketGeneric::Suback(suback) => {
                    let reason_code = suback.first_reason_code();
                    if reason_code.is_error() {
                        return Err(ClientError::Subscribe(*reason_code));
                    }

                    let ack_packet_identifier = suback.packet_identifier();

                    match &self.pending_suback_identifier {
                        Some(expected_packet_identifier) => {
                            if expected_packet_identifier == ack_packet_identifier {
                                self.pending_suback_identifier = None;
                                Ok(true)
                            } else {
                                Err(ClientError::UnexpectedSubackPacketIdentifier)
                            }
                        }
                        None => Err(ClientError::UnexpectedSuback),
                    }
                }
                PacketGeneric::Unsuback(unsuback) => {
                    let reason_code = unsuback.first_reason_code();
                    if reason_code.is_error() {
                        return Err(ClientError::Unsubscribe(*reason_code));
                    }

                    let ack_packet_identifier = unsuback.packet_identifier();
                    match &self.pending_unsuback_identifier {
                        Some(expected_packet_identifier) => {
                            if expected_packet_identifier == ack_packet_identifier {
                                self.pending_unsuback_identifier = None;
                                Ok(true)
                            } else {
                                Err(ClientError::UnexpectedUnsubackPacketIdentifier)
                            }
                        }
                        None => Err(ClientError::UnexpectedUnsuback),
                    }
                }
                PacketGeneric::Pingresp(_pingresp) => {
                    if self.pending_ping_count > 0 {
                        self.pending_ping_count -= 1;
                        Ok(true)
                    } else {
                        Err(ClientError::UnexpectedPingresp)
                    }
                }
                PacketGeneric::Disconnect(_) => Err(ClientError::Disconnect),
                PacketGeneric::Connect(_) => Err(ClientError::ServerOnlyMessageReceived),
                PacketGeneric::Pubrec(_) => Err(ClientError::ServerOnlyMessageReceived),
                PacketGeneric::Pubrel(_) => Err(ClientError::ServerOnlyMessageReceived),
                PacketGeneric::Pubcomp(_) => Err(ClientError::ServerOnlyMessageReceived),
                PacketGeneric::Subscribe(_) => Err(ClientError::ServerOnlyMessageReceived),
                PacketGeneric::Unsubscribe(_) => Err(ClientError::ServerOnlyMessageReceived),
                PacketGeneric::Pingreq(_) => Err(ClientError::ServerOnlyMessageReceived),
                PacketGeneric::Auth(_) => Err(ClientError::ServerOnlyMessageReceived),
            }
        } else {
            Ok(false)
        }
    }
}
