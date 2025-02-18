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
        puback::Puback,
        publish::Publish,
        subscribe::{Subscribe, SubscriptionRequest},
        unsubscribe::Unsubscribe,
    },
};

/// [ClientState] error
#[derive(Debug, PartialEq)]
pub enum ClientStateError {
    PacketWrite(PacketWriteError),
    PacketRead(PacketReadError),
    NotIdle,
    Idle,
    AuthNotSupported,
    QoS2NotSupported,
    ReceivedQoS2PublishNotSupported,
    QoS1MessagePending,
    NotConnected,
    ReceiveWhenNotConnectedOrConnecting,
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

pub enum ClientStateReceiveEvent<'a, 'b, const PROPERTIES_N: usize> {
    /// Nothing happened on receive
    None,

    /// A published message was received
    Publish { publish: Publish<'a, PROPERTIES_N> },

    /// A published message was received, and it needs to be acknowledged by sending provided puback to the server
    PublishAndPubAck {
        publish: Publish<'a, PROPERTIES_N>,
        puback: Puback<'b, PROPERTIES_N>,
    },

    /// A subscription was granted but was at lower qos than the maximum requested
    /// This may or may not require action depending on client requirements -
    /// it means that the given subscription will receive published messages at
    /// only the granted qos - if the requested maximum qos was absolutely required
    /// then the client could respond by showing an error to the user stating the
    /// server is incompatible, or possibly trying to unsubscribe and resubscribe,
    /// assuming this is expected to make any difference with the server(s) in use.
    SubscriptionGrantedBelowMaximumQoS {
        granted_qos: QualityOfService,
        maximum_qos: QualityOfService,
    },

    /// A published message was received at the server, but had no matching subscribers and
    /// so did not reach any receivers
    /// This may or may not require action depending on client requirements / expectations
    /// E.g. if it was expected there would be subscribers, the client could try resending
    /// the message later
    PublishedMessageHadNoMatchingSubscribers,

    /// A [Disconnect] packet was received, it should contain a reason for our disconnection
    Disconnect {
        disconnect: Disconnect<'a, PROPERTIES_N>,
    },
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
        connect: &Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientStateError>;

    /// Produce a packet to disconnect from server, update state
    fn disconnect(&mut self) -> Result<Disconnect<'_, 0>, ClientStateError>;

    /// Produce a packet to ping the server, update state
    fn send_ping(&mut self) -> Result<Pingreq, ClientStateError>;

    /// Receive a packet
    /// This updates the client state, and if anything that might require
    /// action by the caller occurs, a [ClientStateReceiveEvent] is returned.
    /// Errors indicate an invalid packet was received, message_target errored,
    /// or the received packet was unexpected based on our state
    fn receive<'a, 'b, const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
        packet: PacketGeneric<'a, PROPERTIES_N, REQUEST_N>,
    ) -> Result<ClientStateReceiveEvent<'a, 'b, PROPERTIES_N>, ClientStateError>;

    /// Produce a packet to subscribe to a topic by name, update state
    fn subscribe_to_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: &QualityOfService,
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

    /// Move to errored state, no further operations are possible
    /// This must be called if the user of the client state cannot successfully send
    /// a packet produced by this [ClientState]
    fn error(&mut self);
}

#[derive(PartialEq)]
pub enum ConnectionState {
    Idle,
    Connecting,
    Connected,
    Errored,
    Disconnected,
}

pub struct ClientStateNoQueue {
    connection_state: ConnectionState,
    pending_puback_identifier: Option<PacketIdentifier>,
    pending_suback_identifier_and_qos: Option<(PacketIdentifier, QualityOfService)>,
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
            pending_suback_identifier_and_qos: None,
            pending_unsuback_identifier: None,
            pending_ping_count: 0,
        }
    }

    fn any_pending(&self) -> bool {
        self.pending_puback_identifier.is_some()
            || self.pending_suback_identifier_and_qos.is_some()
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
        _connect: &Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientStateError> {
        if self.connection_state == ConnectionState::Idle {
            self.connection_state = ConnectionState::Connecting;
            Ok(())
        } else {
            Err(ClientStateError::NotIdle)
        }
    }

    fn disconnect(&mut self) -> Result<Disconnect<'_, 0>, ClientStateError> {
        if self.connection_state == ConnectionState::Connected {
            self.connection_state = ConnectionState::Disconnected;
            Ok(Disconnect::default())
        } else {
            Err(ClientStateError::NotConnected)
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
                QualityOfService::QoS2 => Err(ClientStateError::QoS2NotSupported),
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
        maximum_qos: &QualityOfService,
    ) -> Result<Subscribe<'b, 0, 0>, ClientStateError> {
        if self.connection_state == ConnectionState::Connected {
            if self.pending_suback_identifier_and_qos.is_some() {
                Err(ClientStateError::SubscriptionPending)
            } else if maximum_qos == &QualityOfService::QoS2 {
                Err(ClientStateError::QoS2NotSupported)
            } else {
                let first_request = SubscriptionRequest::new(topic_name, *maximum_qos);
                let subscribe: Subscribe<'_, 0, 0> = Subscribe::new(
                    Self::SUBSCRIBE_PACKET_IDENTIFIER,
                    first_request,
                    Vec::new(),
                    Vec::new(),
                );

                self.pending_suback_identifier_and_qos =
                    Some((Self::SUBSCRIBE_PACKET_IDENTIFIER, *maximum_qos));

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

    fn receive<'a, 'b, const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
        packet: PacketGeneric<'a, PROPERTIES_N, REQUEST_N>,
    ) -> Result<ClientStateReceiveEvent<'a, 'b, PROPERTIES_N>, ClientStateError> {
        if self.connection_state != ConnectionState::Connected
            && self.connection_state != ConnectionState::Connecting
        {
            return Err(ClientStateError::ReceiveWhenNotConnectedOrConnecting);
        }

        match packet {
            PacketGeneric::Publish(publish) => match publish.publish_packet_identifier() {
                PublishPacketIdentifier::None => Ok(ClientStateReceiveEvent::Publish { publish }),
                PublishPacketIdentifier::Qos1(packet_identifier) => {
                    let puback =
                        Puback::new(*packet_identifier, PublishReasonCode::Success, Vec::new());
                    Ok(ClientStateReceiveEvent::PublishAndPubAck { publish, puback })
                }
                PublishPacketIdentifier::Qos2(_) => {
                    Err(ClientStateError::ReceivedQoS2PublishNotSupported)
                }
            },
            PacketGeneric::Connack(connack) => match connack.reason_code() {
                ConnectReasonCode::Success => {
                    self.connection_state = ConnectionState::Connected;
                    Ok(ClientStateReceiveEvent::None)
                }
                reason_code => Err(ClientStateError::Connect(*reason_code)),
            },
            PacketGeneric::Puback(puback) => {
                let reason_code = puback.reason_code();
                if reason_code.is_error() {
                    return Err(ClientStateError::Publish(*reason_code));
                }

                let ack_packet_identifier = puback.packet_identifier();
                match &self.pending_puback_identifier {
                    Some(expected_packet_identifier) => {
                        if expected_packet_identifier == ack_packet_identifier {
                            self.pending_puback_identifier = None;

                            if reason_code == &PublishReasonCode::NoMatchingSubscribers {
                                Ok(ClientStateReceiveEvent::None)
                            } else {
                                Ok(ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers)
                            }
                        } else {
                            Err(ClientStateError::UnexpectedPubackPacketIdentifier)
                        }
                    }
                    None => Err(ClientStateError::UnexpectedPuback),
                }
            }
            PacketGeneric::Suback(suback) => {
                let reason_code = suback.first_reason_code();
                let granted_qos = match reason_code {
                    SubscribeReasonCode::Success => QualityOfService::QoS0,
                    SubscribeReasonCode::GrantedQoS1 => QualityOfService::QoS1,
                    SubscribeReasonCode::GrantedQoS2 => QualityOfService::QoS2,
                    err => return Err(ClientStateError::Subscribe(*err)),
                };

                let ack_packet_identifier = suback.packet_identifier();

                match &self.pending_suback_identifier_and_qos {
                    Some((expected_packet_identifier, maximum_qos)) => {
                        if expected_packet_identifier == ack_packet_identifier {
                            let maximum_qos = *maximum_qos;
                            self.pending_suback_identifier_and_qos = None;

                            if granted_qos != maximum_qos {
                                Ok(
                                    ClientStateReceiveEvent::SubscriptionGrantedBelowMaximumQoS {
                                        granted_qos,
                                        maximum_qos,
                                    },
                                )
                            } else {
                                Ok(ClientStateReceiveEvent::None)
                            }
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
                            Ok(ClientStateReceiveEvent::None)
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
                    Ok(ClientStateReceiveEvent::None)
                } else {
                    Err(ClientStateError::UnexpectedPingresp)
                }
            }
            PacketGeneric::Disconnect(disconnect) => {
                Ok(ClientStateReceiveEvent::Disconnect { disconnect })
            }
            PacketGeneric::Auth(_auth) => Err(ClientStateError::AuthNotSupported),
            PacketGeneric::Connect(_)
            | PacketGeneric::Pubrec(_)
            | PacketGeneric::Pubrel(_)
            | PacketGeneric::Pubcomp(_)
            | PacketGeneric::Subscribe(_)
            | PacketGeneric::Unsubscribe(_)
            | PacketGeneric::Pingreq(_) => Err(ClientStateError::ServerOnlyMessageReceived),
        }
    }

    fn pending_ping_count(&self) -> u32 {
        self.pending_ping_count
    }

    fn error(&mut self) {
        self.connection_state = ConnectionState::Errored;
    }
}
