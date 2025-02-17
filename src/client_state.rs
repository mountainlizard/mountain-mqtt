use heapless::Vec;

use crate::{
    data::{
        packet_identifier::{PacketIdentifier, PublishPacketIdentifier},
        quality_of_service::QualityOfService,
        reason_code::{
            ConnectReasonCode, PublishReasonCode, SubscribeReasonCode, UnsubscribeReasonCode,
        },
    },
    error::{PacketReadError, PacketWriteError},
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
pub enum ClientStateError {
    PacketWrite(PacketWriteError),
    PacketRead(PacketReadError),
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
    Connect(ConnectReasonCode),
    Subscribe(SubscribeReasonCode),
    Publish(PublishReasonCode),
    Unsubscribe(UnsubscribeReasonCode),
}

impl From<PacketWriteError> for ClientStateError {
    fn from(value: PacketWriteError) -> Self {
        ClientStateError::PacketWrite(value)
    }
}

impl From<PacketReadError> for ClientStateError {
    fn from(value: PacketReadError) -> Self {
        ClientStateError::PacketRead(value)
    }
}

/// Tracks the state of a simple MQTT client, allowing for tracking acknowledgements, and producing the necessary packets for basic actions
#[allow(async_fn_in_trait)]
pub trait ClientState {
    /// Returns true when the state indicates we require a response to a sent
    /// packet - in this case you must receive packets and provide them to
    /// `receive` until `waiting_for_responses` returns false.
    /// The exception is that it is possible to call `disconnect`,
    /// `send_ping` or `send_message` (with a quality of service of 0),
    /// while `waiting_for_responses` is true.
    fn waiting_for_responses(&self) -> bool;

    /// Return the number of pings that have been sent, but not responded to
    /// This can be used as a way of detecting a dead connection or server
    fn pending_ping_count(&self) -> u32;

    /// Update state based on a packet used to connect to server
    /// Call this after connect packet has been successfully sent.
    fn connect<const PROPERTIES_N: usize>(
        &mut self,
        connect: Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientStateError>;

    /// Produce a packet to disconnect from server, update state
    fn disconnect(&mut self) -> Result<Disconnect<'_, 0>, ClientStateError>;

    /// Produce a packet to ping the server, update state
    fn send_ping(&mut self) -> Result<Pingreq, ClientStateError>;

    /// Receive a packet
    /// This updates the client state, and if the packet is a Publish message,
    /// returns this for handling
    /// Errors indicate an invalid packet was received, message_target errored,
    /// or the received packet was unexpected based on our state
    fn receive<'a, const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
        packet: PacketGeneric<'a, PROPERTIES_N, REQUEST_N>,
    ) -> Result<Option<Publish<'a, PROPERTIES_N>>, ClientStateError>;

    /// Produce a packet to subscribe to a topic by name, update state
    fn subscribe_to_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
    ) -> Result<Subscribe<'b, 0, 0>, ClientStateError>;

    /// Produce a packet to unsubscribe from a topic by name, update state
    fn unsubscribe_from_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
    ) -> Result<Unsubscribe<'b, 0, 0>, ClientStateError>;

    /// Produce a packet to publish to a given topic, update state
    fn send_message<'b>(
        &'b mut self,
        topic_name: &'b str,
        message: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<Publish<'b, 0>, ClientStateError>;
}

#[derive(PartialEq)]
pub enum ConnectionState {
    Idle,
    Connecting,
    Connected,
}

pub struct ClientStateNoQueue {
    connection_state: ConnectionState,
    pending_puback_identifier: Option<PacketIdentifier>,
    pending_suback_identifier: Option<PacketIdentifier>,
    pending_unsuback_identifier: Option<PacketIdentifier>,
    pending_ping_count: u32,
}

impl ClientStateNoQueue {
    const PUBLISH_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(1);
    const SUBSCRIBE_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(2);
    const UNSUBSCRIBE_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(3);

    pub fn new() -> Self {
        Self {
            connection_state: ConnectionState::Idle,
            pending_puback_identifier: None,
            pending_suback_identifier: None,
            pending_unsuback_identifier: None,
            pending_ping_count: 0,
        }
    }

    fn any_pending(&self) -> bool {
        self.pending_puback_identifier.is_some()
            || self.pending_suback_identifier.is_some()
            || self.pending_unsuback_identifier.is_some()
    }
}

impl Default for ClientStateNoQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientState for ClientStateNoQueue {
    fn waiting_for_responses(&self) -> bool {
        self.connection_state == ConnectionState::Connecting
            || (self.connection_state == ConnectionState::Connected && self.any_pending())
    }

    fn connect<const PROPERTIES_N: usize>(
        &mut self,
        _connect: Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientStateError> {
        if self.connection_state == ConnectionState::Idle {
            self.connection_state = ConnectionState::Connecting;
            Ok(())
        } else {
            Err(ClientStateError::NotIdle)
        }
    }

    fn disconnect(&mut self) -> Result<Disconnect<'_, 0>, ClientStateError> {
        if self.connection_state != ConnectionState::Idle {
            self.connection_state = ConnectionState::Idle;
            Ok(Disconnect::default())
        } else {
            Err(ClientStateError::Idle)
        }
    }

    fn send_message<'b>(
        &'b mut self,
        topic_name: &'b str,
        message: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<Publish<'b, 0>, ClientStateError> {
        if self.connection_state == ConnectionState::Connected {
            let publish_packet_identifier = match qos {
                QualityOfService::QoS0 => Ok(PublishPacketIdentifier::None),
                QualityOfService::QoS1 if self.pending_puback_identifier.is_some() => {
                    Err(ClientStateError::QoS1MessagePending)
                }
                QualityOfService::QoS1 => Ok(PublishPacketIdentifier::Qos1(
                    Self::PUBLISH_PACKET_IDENTIFIER,
                )),
                QualityOfService::QoS2 => Err(ClientStateError::UnsupportedFeature),
            }?;

            let publish: Publish<'_, 0> = Publish::new(
                false,
                retain,
                topic_name,
                publish_packet_identifier,
                message,
                Vec::new(),
            );

            if qos == QualityOfService::QoS1 {
                self.pending_puback_identifier = Some(Self::PUBLISH_PACKET_IDENTIFIER);
            }

            Ok(publish)
        } else {
            Err(ClientStateError::NotConnected)
        }
    }

    fn subscribe_to_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
    ) -> Result<Subscribe<'b, 0, 0>, ClientStateError> {
        if self.connection_state == ConnectionState::Connected {
            if self.pending_suback_identifier.is_some() {
                Err(ClientStateError::SubscriptionPending)
            } else {
                let first_request = SubscriptionRequest::new(topic_name, QualityOfService::QoS0);
                let subscribe: Subscribe<'_, 0, 0> = Subscribe::new(
                    Self::SUBSCRIBE_PACKET_IDENTIFIER,
                    first_request,
                    Vec::new(),
                    Vec::new(),
                );

                self.pending_suback_identifier = Some(Self::SUBSCRIBE_PACKET_IDENTIFIER);

                Ok(subscribe)
            }
        } else {
            Err(ClientStateError::NotConnected)
        }
    }

    fn unsubscribe_from_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
    ) -> Result<Unsubscribe<'b, 0, 0>, ClientStateError> {
        if self.connection_state == ConnectionState::Connected {
            if self.pending_unsuback_identifier.is_some() {
                Err(ClientStateError::UnsubscriptionPending)
            } else {
                let unsubscribe: Unsubscribe<'_, 0, 0> = Unsubscribe::new(
                    Self::UNSUBSCRIBE_PACKET_IDENTIFIER,
                    topic_name,
                    Vec::new(),
                    Vec::new(),
                );

                self.pending_unsuback_identifier = Some(Self::UNSUBSCRIBE_PACKET_IDENTIFIER);
                Ok(unsubscribe)
            }
        } else {
            Err(ClientStateError::NotConnected)
        }
    }

    fn send_ping(&mut self) -> Result<Pingreq, ClientStateError> {
        if self.connection_state == ConnectionState::Connected {
            self.pending_ping_count += 1;
            Ok(Pingreq::default())
        } else {
            Err(ClientStateError::NotConnected)
        }
    }

    fn receive<'a, const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
        packet: PacketGeneric<'a, PROPERTIES_N, REQUEST_N>,
    ) -> Result<Option<Publish<'a, PROPERTIES_N>>, ClientStateError> {
        match packet {
            PacketGeneric::Publish(publish) => Ok(Some(publish)),
            PacketGeneric::Connack(connack) => match connack.reason_code() {
                ConnectReasonCode::Success => {
                    self.connection_state = ConnectionState::Connected;
                    Ok(None)
                }
                reason_code => Err(ClientStateError::Connect(*reason_code)),
            },
            PacketGeneric::Puback(puback) => {
                let reason_code = puback.reason_code();
                // TODO: Should we do anything about NoMatchingSubscribers?
                if reason_code.is_error() {
                    return Err(ClientStateError::Publish(*reason_code));
                }

                let ack_packet_identifier = puback.packet_identifier();
                match &self.pending_puback_identifier {
                    Some(expected_packet_identifier) => {
                        if expected_packet_identifier == ack_packet_identifier {
                            self.pending_puback_identifier = None;
                            Ok(None)
                        } else {
                            Err(ClientStateError::UnexpectedPubackPacketIdentifier)
                        }
                    }
                    None => Err(ClientStateError::UnexpectedPuback),
                }
            }
            PacketGeneric::Suback(suback) => {
                let reason_code = suback.first_reason_code();
                if reason_code.is_error() {
                    return Err(ClientStateError::Subscribe(*reason_code));
                }

                let ack_packet_identifier = suback.packet_identifier();

                match &self.pending_suback_identifier {
                    Some(expected_packet_identifier) => {
                        if expected_packet_identifier == ack_packet_identifier {
                            self.pending_suback_identifier = None;
                            Ok(None)
                        } else {
                            Err(ClientStateError::UnexpectedSubackPacketIdentifier)
                        }
                    }
                    None => Err(ClientStateError::UnexpectedSuback),
                }
            }
            PacketGeneric::Unsuback(unsuback) => {
                let reason_code = unsuback.first_reason_code();
                if reason_code.is_error() {
                    return Err(ClientStateError::Unsubscribe(*reason_code));
                }

                let ack_packet_identifier = unsuback.packet_identifier();
                match &self.pending_unsuback_identifier {
                    Some(expected_packet_identifier) => {
                        if expected_packet_identifier == ack_packet_identifier {
                            self.pending_unsuback_identifier = None;
                            Ok(None)
                        } else {
                            Err(ClientStateError::UnexpectedUnsubackPacketIdentifier)
                        }
                    }
                    None => Err(ClientStateError::UnexpectedUnsuback),
                }
            }
            PacketGeneric::Pingresp(_pingresp) => {
                if self.pending_ping_count > 0 {
                    self.pending_ping_count -= 1;
                    Ok(None)
                } else {
                    Err(ClientStateError::UnexpectedPingresp)
                }
            }
            PacketGeneric::Disconnect(_) => Err(ClientStateError::Disconnect),
            PacketGeneric::Connect(_) => Err(ClientStateError::ServerOnlyMessageReceived),
            PacketGeneric::Pubrec(_) => Err(ClientStateError::ServerOnlyMessageReceived),
            PacketGeneric::Pubrel(_) => Err(ClientStateError::ServerOnlyMessageReceived),
            PacketGeneric::Pubcomp(_) => Err(ClientStateError::ServerOnlyMessageReceived),
            PacketGeneric::Subscribe(_) => Err(ClientStateError::ServerOnlyMessageReceived),
            PacketGeneric::Unsubscribe(_) => Err(ClientStateError::ServerOnlyMessageReceived),
            PacketGeneric::Pingreq(_) => Err(ClientStateError::ServerOnlyMessageReceived),
            PacketGeneric::Auth(_) => Err(ClientStateError::ServerOnlyMessageReceived),
        }
    }

    fn pending_ping_count(&self) -> u32 {
        self.pending_ping_count
    }
}
