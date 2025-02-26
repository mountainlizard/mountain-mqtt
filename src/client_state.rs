use core::fmt::{Display, Formatter};

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
    ClientIsWaitingForResponse,
    NotConnected,
    ReceiveWhenNotConnectedOrConnecting,
    UnexpectedPuback,
    UnexpectedPubackPacketIdentifier,
    UnexpectedSuback,
    UnexpectedSubackPacketIdentifier,
    UnexpectedUnsuback,
    UnexpectedUnsubackPacketIdentifier,
    UnexpectedPingresp,
    Disconnect,
    ServerOnlyMessageReceived,
    ReceivedPacketOtherThanConnackWhenConnecting,
    ReceivedConnackWhenNotConnecting,
    Connect(ConnectReasonCode),
    Subscribe(SubscribeReasonCode),
    Publish(PublishReasonCode),
    Unsubscribe(UnsubscribeReasonCode),
}

impl Display for ClientStateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PacketWrite(e) => write!(f, "PacketWrite({})", e),
            Self::PacketRead(e) => write!(f, "PacketRead({})", e),
            Self::NotIdle => write!(f, "NotIdle"),
            Self::Idle => write!(f, "Idle"),
            Self::AuthNotSupported => write!(f, "AuthNotSupported"),
            Self::QoS2NotSupported => write!(f, "QoS2NotSupported"),
            Self::ReceivedQoS2PublishNotSupported => write!(f, "ReceivedQoS2PublishNotSupported"),
            Self::ClientIsWaitingForResponse => write!(f, "QoS1MessagePending"),
            Self::NotConnected => write!(f, "NotConnected"),
            Self::ReceiveWhenNotConnectedOrConnecting => {
                write!(f, "ReceiveWhenNotConnectedOrConnecting")
            }
            Self::UnexpectedPuback => write!(f, "UnexpectedPuback"),
            Self::UnexpectedPubackPacketIdentifier => write!(f, "UnexpectedPubackPacketIdentifier"),
            Self::UnexpectedSuback => write!(f, "UnexpectedSuback"),
            Self::UnexpectedSubackPacketIdentifier => write!(f, "UnexpectedSubackPacketIdentifier"),
            Self::UnexpectedUnsuback => write!(f, "UnexpectedUnsuback"),
            Self::UnexpectedUnsubackPacketIdentifier => {
                write!(f, "UnexpectedUnsubackPacketIdentifier")
            }
            Self::UnexpectedPingresp => write!(f, "UnexpectedPingresp"),
            Self::Disconnect => write!(f, "Disconnect"),
            Self::ServerOnlyMessageReceived => write!(f, "ServerOnlyMessageReceived"),
            Self::Connect(e) => write!(f, "Connect({})", e),
            Self::Subscribe(e) => write!(f, "Subscribe({})", e),
            Self::Publish(e) => write!(f, "Publish({})", e),
            Self::Unsubscribe(e) => write!(f, "Unsubscribe({})", e),
            // Self::Overflow => write!(f, "Overflow"),
            Self::ReceivedPacketOtherThanConnackWhenConnecting => {
                write!(f, "ReceivedPacketOtherThanConnackWhenConnecting")
            }
            Self::ReceivedConnackWhenNotConnecting => write!(f, "ReceivedConnackWhenNotConnecting"),
        }
    }
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

    /// Update state based on a packet used to connect to server
    /// Call this after connect packet has been successfully sent.
    fn connect<const PROPERTIES_N: usize>(
        &mut self,
        connect: &Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientStateError>;

    /// Produce a packet to disconnect from server, update state
    fn disconnect<'b>(&mut self) -> Result<Disconnect<'b, 0>, ClientStateError>;

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
    fn subscribe<'b>(
        &mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<Subscribe<'b, 0, 0>, ClientStateError>;

    /// Produce a packet to unsubscribe from a topic by name, update state
    fn unsubscribe<'b>(
        &mut self,
        topic_name: &'b str,
    ) -> Result<Unsubscribe<'b, 0, 0>, ClientStateError>;

    /// Produce a packet to publish to a given topic, update state
    fn publish<'b>(
        &mut self,
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
pub enum ClientStateNoQueue {
    Idle,
    Connecting,
    Connected(ConnectionData),
    Errored,
    Disconnected,
}

#[derive(PartialEq)]
pub struct ConnectionData {
    pending_ping_count: u32,
    state: ConnectionState,
}

impl Default for ConnectionData {
    fn default() -> Self {
        Self {
            pending_ping_count: 0,
            state: ConnectionState::Idle,
        }
    }
}

#[derive(PartialEq)]
pub enum ConnectionState {
    Idle,
    WaitingForPuback {
        id: PacketIdentifier,
    },
    WaitingForSuback {
        id: PacketIdentifier,
        qos: QualityOfService,
    },
    WaitingForUnsuback {
        id: PacketIdentifier,
    },
}

impl ConnectionState {
    fn is_waiting(&self) -> bool {
        match self {
            ConnectionState::Idle => false,
            ConnectionState::WaitingForPuback { id: _ } => true,
            ConnectionState::WaitingForSuback { id: _, qos: _ } => true,
            ConnectionState::WaitingForUnsuback { id: _ } => true,
        }
    }
}

impl ClientStateNoQueue {
    const PUBLISH_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(1);
    const SUBSCRIBE_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(2);
    const UNSUBSCRIBE_PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(3);

    pub fn new() -> Self {
        Self::Idle
    }
}

impl Default for ClientStateNoQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientState for ClientStateNoQueue {
    fn waiting_for_responses(&self) -> bool {
        match self {
            ClientStateNoQueue::Idle => false,
            ClientStateNoQueue::Connecting => true,
            ClientStateNoQueue::Connected(connection_data) => connection_data.state.is_waiting(),
            ClientStateNoQueue::Errored => false,
            ClientStateNoQueue::Disconnected => false,
        }
    }

    fn connect<const PROPERTIES_N: usize>(
        &mut self,
        _connect: &Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientStateError> {
        match self {
            ClientStateNoQueue::Idle => {
                *self = Self::Connecting;
                Ok(())
            }
            _ => Err(ClientStateError::NotIdle),
        }
    }

    fn disconnect<'b>(&mut self) -> Result<Disconnect<'b, 0>, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(_d) => {
                *self = Self::Disconnected;
                Ok(Disconnect::default())
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn publish<'b>(
        &mut self,
        topic_name: &'b str,
        message: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<Publish<'b, 0>, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionData {
                pending_ping_count: _,
                state,
            }) => {
                let publish_packet_identifier = match qos {
                    QualityOfService::QoS0 => Ok(PublishPacketIdentifier::None),
                    QualityOfService::QoS1 if state.is_waiting() => {
                        Err(ClientStateError::ClientIsWaitingForResponse)
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
                    *state = ConnectionState::WaitingForPuback {
                        id: Self::PUBLISH_PACKET_IDENTIFIER,
                    };
                }

                Ok(publish)
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn subscribe<'b>(
        &mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<Subscribe<'b, 0, 0>, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionData {
                pending_ping_count: _,
                state,
            }) => {
                if state.is_waiting() {
                    Err(ClientStateError::ClientIsWaitingForResponse)
                } else if maximum_qos == QualityOfService::QoS2 {
                    Err(ClientStateError::QoS2NotSupported)
                } else {
                    let first_request = SubscriptionRequest::new(topic_name, maximum_qos);
                    let subscribe: Subscribe<'_, 0, 0> = Subscribe::new(
                        Self::SUBSCRIBE_PACKET_IDENTIFIER,
                        first_request,
                        Vec::new(),
                        Vec::new(),
                    );

                    *state = ConnectionState::WaitingForSuback {
                        id: Self::SUBSCRIBE_PACKET_IDENTIFIER,
                        qos: maximum_qos,
                    };

                    Ok(subscribe)
                }
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn unsubscribe<'b>(
        &mut self,
        topic_name: &'b str,
    ) -> Result<Unsubscribe<'b, 0, 0>, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionData {
                pending_ping_count: _,
                state,
            }) => {
                if state.is_waiting() {
                    Err(ClientStateError::ClientIsWaitingForResponse)
                } else {
                    let unsubscribe: Unsubscribe<'_, 0, 0> = Unsubscribe::new(
                        Self::UNSUBSCRIBE_PACKET_IDENTIFIER,
                        topic_name,
                        Vec::new(),
                        Vec::new(),
                    );

                    *state = ConnectionState::WaitingForUnsuback {
                        id: Self::UNSUBSCRIBE_PACKET_IDENTIFIER,
                    };

                    Ok(unsubscribe)
                }
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn send_ping(&mut self) -> Result<Pingreq, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionData {
                pending_ping_count,
                state: _,
            }) => {
                *pending_ping_count += 1;
                Ok(Pingreq::default())
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn receive<'a, 'b, const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
        packet: PacketGeneric<'a, PROPERTIES_N, REQUEST_N>,
    ) -> Result<ClientStateReceiveEvent<'a, 'b, PROPERTIES_N>, ClientStateError> {
        match self {
            // If we are connecting, we only expect a Connack packet
            ClientStateNoQueue::Connecting => match packet {
                PacketGeneric::Connack(connack) => match connack.reason_code() {
                    ConnectReasonCode::Success => {
                        *self = Self::Connected(ConnectionData::default());
                        Ok(ClientStateReceiveEvent::None)
                    }
                    reason_code => Err(ClientStateError::Connect(*reason_code)),
                },
                _ => Err(ClientStateError::ReceivedPacketOtherThanConnackWhenConnecting),
            },

            // If we are connected, we handle all client packets other than Connack
            ClientStateNoQueue::Connected(ConnectionData {
                pending_ping_count,
                state,
            }) => match packet {
                PacketGeneric::Publish(publish) => match publish.publish_packet_identifier() {
                    PublishPacketIdentifier::None => {
                        Ok(ClientStateReceiveEvent::Publish { publish })
                    }
                    PublishPacketIdentifier::Qos1(packet_identifier) => {
                        let puback =
                            Puback::new(*packet_identifier, PublishReasonCode::Success, Vec::new());
                        Ok(ClientStateReceiveEvent::PublishAndPubAck { publish, puback })
                    }
                    PublishPacketIdentifier::Qos2(_) => {
                        Err(ClientStateError::ReceivedQoS2PublishNotSupported)
                    }
                },

                PacketGeneric::Puback(puback) => {
                    let ack_id = puback.packet_identifier();
                    match state {
                        ConnectionState::WaitingForPuback { id } if id == ack_id => {
                            *state = ConnectionState::Idle;

                            let reason_code = puback.reason_code();
                            if reason_code.is_error() {
                                Err(ClientStateError::Publish(*reason_code))
                            } else if reason_code == &PublishReasonCode::NoMatchingSubscribers {
                                Ok(ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers)
                            } else {
                                Ok(ClientStateReceiveEvent::None)
                            }
                        }
                        ConnectionState::WaitingForPuback { id: _ } => {
                            Err(ClientStateError::UnexpectedPubackPacketIdentifier)
                        }
                        _ => Err(ClientStateError::UnexpectedPuback),
                    }
                }

                PacketGeneric::Suback(suback) => {
                    let ack_id = suback.packet_identifier();

                    match state {
                        ConnectionState::WaitingForSuback { id, qos } if id == ack_id => {
                            let maximum_qos = *qos;
                            *state = ConnectionState::Idle;

                            let reason_code = suback.first_reason_code();
                            let granted_qos = match reason_code {
                                SubscribeReasonCode::Success => QualityOfService::QoS0,
                                SubscribeReasonCode::GrantedQoS1 => QualityOfService::QoS1,
                                SubscribeReasonCode::GrantedQoS2 => QualityOfService::QoS2,
                                err => return Err(ClientStateError::Subscribe(*err)),
                            };

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
                        }
                        ConnectionState::WaitingForSuback { id: _, qos: _ } => {
                            Err(ClientStateError::UnexpectedSubackPacketIdentifier)
                        }
                        _ => Err(ClientStateError::UnexpectedSuback),
                    }
                }
                PacketGeneric::Unsuback(unsuback) => {
                    let ack_id = unsuback.packet_identifier();

                    match state {
                        ConnectionState::WaitingForUnsuback { id } if id == ack_id => {
                            *state = ConnectionState::Idle;

                            let reason_code = unsuback.first_reason_code();
                            if reason_code.is_error() {
                                Err(ClientStateError::Unsubscribe(*reason_code))
                            } else {
                                Ok(ClientStateReceiveEvent::None)
                            }
                        }
                        ConnectionState::WaitingForUnsuback { id: _ } => {
                            Err(ClientStateError::UnexpectedUnsubackPacketIdentifier)
                        }
                        _ => Err(ClientStateError::UnexpectedUnsuback),
                    }
                }
                PacketGeneric::Pingresp(_pingresp) => {
                    if *pending_ping_count > 0 {
                        *pending_ping_count -= 1;
                        Ok(ClientStateReceiveEvent::None)
                    } else {
                        Err(ClientStateError::UnexpectedPingresp)
                    }
                }
                PacketGeneric::Disconnect(disconnect) => {
                    Ok(ClientStateReceiveEvent::Disconnect { disconnect })
                }
                PacketGeneric::Connack(_) => {
                    Err(ClientStateError::ReceivedConnackWhenNotConnecting)
                }
                PacketGeneric::Auth(_auth) => Err(ClientStateError::AuthNotSupported),
                PacketGeneric::Connect(_)
                | PacketGeneric::Pubrec(_)
                | PacketGeneric::Pubrel(_)
                | PacketGeneric::Pubcomp(_)
                | PacketGeneric::Subscribe(_)
                | PacketGeneric::Unsubscribe(_)
                | PacketGeneric::Pingreq(_) => Err(ClientStateError::ServerOnlyMessageReceived),
            },
            _ => Err(ClientStateError::ReceiveWhenNotConnectedOrConnecting),
        }
    }

    fn error(&mut self) {
        *self = Self::Errored;
    }
}
