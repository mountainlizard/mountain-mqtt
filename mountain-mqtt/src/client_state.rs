use core::fmt::{Display, Formatter};

use heapless::Vec;

use crate::{
    data::{
        packet_identifier::{PacketIdentifier, PublishPacketIdentifier},
        property::{ConnackProperty, Property, PublishProperty},
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
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ClientStateError {
    PacketWrite(PacketWriteError),
    PacketRead(PacketReadError),
    NotIdle,
    AuthNotSupported,
    Qos2NotSupported,
    MultipleSubscriptionRequestsNotSupported,
    ReceivedQos2PublishNotSupported,
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
    ReceivedPacketOtherThanConnackOrAuthWhenConnecting,
    ReceivedConnackWhenNotConnecting,
    UnexpectedSessionPresentForCleanStart,
    Connect(ConnectReasonCode),
    Subscribe(SubscribeReasonCode),
    Publish(PublishReasonCode),
    Unsubscribe(UnsubscribeReasonCode),
}

#[cfg(feature = "defmt")]
impl defmt::Format for ClientStateError {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::PacketWrite(e) => defmt::write!(f, "PacketWrite({})", e),
            Self::PacketRead(e) => defmt::write!(f, "PacketRead({})", e),
            Self::NotIdle => defmt::write!(f, "NotIdle"),
            Self::AuthNotSupported => defmt::write!(f, "AuthNotSupported"),
            Self::Qos2NotSupported => defmt::write!(f, "Qos2NotSupported"),
            Self::MultipleSubscriptionRequestsNotSupported => {
                defmt::write!(f, "MultipleSubscriptionRequestsNotSupported")
            }
            Self::ReceivedQos2PublishNotSupported => {
                defmt::write!(f, "ReceivedQos2PublishNotSupported")
            }
            Self::ClientIsWaitingForResponse => defmt::write!(f, "ClientIsWaitingForResponse"),
            Self::NotConnected => defmt::write!(f, "NotConnected"),
            Self::ReceiveWhenNotConnectedOrConnecting => {
                defmt::write!(f, "ReceiveWhenNotConnectedOrConnecting")
            }
            Self::UnexpectedPuback => defmt::write!(f, "UnexpectedPuback"),
            Self::UnexpectedPubackPacketIdentifier => {
                defmt::write!(f, "UnexpectedPubackPacketIdentifier")
            }
            Self::UnexpectedSuback => defmt::write!(f, "UnexpectedSuback"),
            Self::UnexpectedSubackPacketIdentifier => {
                defmt::write!(f, "UnexpectedSubackPacketIdentifier")
            }
            Self::UnexpectedUnsuback => defmt::write!(f, "UnexpectedUnsuback"),
            Self::UnexpectedUnsubackPacketIdentifier => {
                defmt::write!(f, "UnexpectedUnsubackPacketIdentifier")
            }
            Self::UnexpectedPingresp => defmt::write!(f, "UnexpectedPingresp"),
            Self::Disconnect => defmt::write!(f, "Disconnect"),
            Self::ServerOnlyMessageReceived => defmt::write!(f, "ServerOnlyMessageReceived"),
            Self::ReceivedPacketOtherThanConnackOrAuthWhenConnecting => {
                defmt::write!(f, "ReceivedPacketOtherThanConnackOrAuthWhenConnecting")
            }
            Self::ReceivedConnackWhenNotConnecting => {
                defmt::write!(f, "ReceivedConnackWhenNotConnecting")
            }
            Self::UnexpectedSessionPresentForCleanStart => {
                defmt::write!(f, "UnexpectedSessionPresentForCleanStart")
            }
            Self::Connect(r) => defmt::write!(f, "Connect({})", r),
            Self::Subscribe(r) => defmt::write!(f, "Subscribe({})", r),
            Self::Publish(r) => defmt::write!(f, "Publish({})", r),
            Self::Unsubscribe(r) => defmt::write!(f, "Unsubscribe({})", r),
        }
    }
}

impl Display for ClientStateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PacketWrite(e) => write!(f, "PacketWrite({})", e),
            Self::PacketRead(e) => write!(f, "PacketRead({})", e),
            Self::NotIdle => write!(f, "NotIdle"),
            Self::AuthNotSupported => write!(f, "AuthNotSupported"),
            Self::Qos2NotSupported => write!(f, "Qos2NotSupported"),
            Self::MultipleSubscriptionRequestsNotSupported => {
                write!(f, "MultipleSubscriptionRequestsNotSupported")
            }
            Self::ReceivedQos2PublishNotSupported => write!(f, "ReceivedQos2PublishNotSupported"),
            Self::ClientIsWaitingForResponse => write!(f, "ClientIsWaitingForResponse"),
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
            Self::ReceivedPacketOtherThanConnackOrAuthWhenConnecting => {
                write!(f, "ReceivedPacketOtherThanConnackWhenConnecting")
            }
            Self::ReceivedConnackWhenNotConnecting => write!(f, "ReceivedConnackWhenNotConnecting"),
            Self::UnexpectedSessionPresentForCleanStart => {
                write!(f, "UnexpectedSessionPresentForCleanStart")
            }
        }
    }
}

pub enum ClientStateReceiveEvent<'a, 'b, const P: usize> {
    /// Client received an acknowledgement/response for a previous message sent
    /// to the server (e.g. Connack, Puback, Suback, Unsuback, Pingresp)
    /// These are all handled internally by the state, so do not need an external
    /// response
    Ack,

    /// A published message was received
    Publish { publish: Publish<'a, P> },

    /// A published message was received, and it needs to be acknowledged by sending provided puback to the server
    PublishAndPuback {
        publish: Publish<'a, P>,
        puback: Puback<'b, P>,
    },

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

    /// A [Disconnect] packet was received, it should contain a reason for our disconnection
    Disconnect { disconnect: Disconnect<'a, P> },
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
    fn connect<const P: usize, const W: usize>(
        &mut self,
        connect: &Connect<'_, P, W>,
    ) -> Result<(), ClientStateError>;

    /// Produce a packet to disconnect from server, update state
    fn disconnect<'b>(&mut self) -> Result<Disconnect<'b, 0>, ClientStateError>;

    /// Produce a packet to ping the server, update state
    fn send_ping(&mut self) -> Result<Pingreq, ClientStateError>;

    /// If connected, the number of pings that have been sent, but not responded
    /// to, otherwise 0.
    fn pending_ping_count(&self) -> u32;

    /// Receive a packet
    /// This updates the client state, and if anything that might require
    /// action by the caller occurs, a [ClientStateReceiveEvent] is returned.
    /// Errors indicate an invalid packet was received, message_target errored,
    /// or the received packet was unexpected based on our state
    fn receive<'a, 'b, const P: usize, const W: usize, const S: usize>(
        &mut self,
        packet: PacketGeneric<'a, P, W, S>,
    ) -> Result<ClientStateReceiveEvent<'a, 'b, P>, ClientStateError>;

    /// Produce a packet to subscribe to a topic by name, update state
    fn subscribe<'b>(
        &mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<Subscribe<'b, 0, 0>, ClientStateError> {
        let packet = self.subscribe_packet(topic_name, maximum_qos)?;
        self.subscribe_update(&packet)?;
        Ok(packet)
    }

    /// Produce a packet to subscribe to a topic by name, this does not update
    /// the state - call [`Self::subscribe_update`] after sending the packet.
    fn subscribe_packet<'b>(
        &mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<Subscribe<'b, 0, 0>, ClientStateError>;

    /// Update the state of the client after sending a subscribe packet
    fn subscribe_update<'b, const P: usize, const S: usize>(
        &mut self,
        packet: &Subscribe<'b, P, S>,
    ) -> Result<(), ClientStateError>;

    /// Produce a packet to unsubscribe from a topic by name, update state
    fn unsubscribe<'b>(
        &mut self,
        topic_name: &'b str,
    ) -> Result<Unsubscribe<'b, 0, 0>, ClientStateError> {
        let packet = self.unsubscribe_packet(topic_name)?;
        self.unsubscribe_update(&packet)?;
        Ok(packet)
    }

    /// Produce a packet to unsubscribe from a topic by name, this does not update
    /// the state - call [`Self::unsubscribe_update`] after sending the packet.
    fn unsubscribe_packet<'b>(
        &mut self,
        topic_name: &'b str,
    ) -> Result<Unsubscribe<'b, 0, 0>, ClientStateError>;

    /// Update the state of the client after sending an unsubscribe packet
    fn unsubscribe_update<'b, const P: usize, const S: usize>(
        &mut self,
        packet: &Unsubscribe<'b, P, S>,
    ) -> Result<(), ClientStateError>;

    /// Produce a packet to publish to a given topic, update state, with no properties
    fn publish<'b>(
        &mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<Publish<'b, 0>, ClientStateError> {
        self.publish_with_properties(topic_name, payload, qos, retain, Vec::new())
    }

    /// Produce a packet to publish to a given topic, update state, with properties
    fn publish_with_properties<'b, const P: usize>(
        &mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
        properties: Vec<PublishProperty<'b>, P>,
    ) -> Result<Publish<'b, P>, ClientStateError>;

    /// Move to errored state, no further operations are possible
    /// This must be called if the user of the client state cannot successfully send
    /// a packet produced by this [ClientState]
    fn error(&mut self);
}

#[derive(PartialEq)]
pub enum ClientStateNoQueue {
    Idle,
    Connecting(RequestedConnectionInfo),
    Connected(ConnectionState),
    Errored,
    Disconnected,
}

#[derive(PartialEq)]
pub struct RequestedConnectionInfo {
    clean_start: bool,
    keep_alive: u16,
}

#[derive(PartialEq)]
pub struct ConnectionInfo {
    pending_ping_count: u32,
    session_present: bool,
    keep_alive: u16,
}

#[derive(PartialEq)]
pub struct ConnectionState {
    info: ConnectionInfo,
    waiting: Waiting,
}

#[derive(PartialEq)]
enum Waiting {
    None,
    ForPuback {
        id: PacketIdentifier,
    },
    ForSuback {
        id: PacketIdentifier,
        qos: QualityOfService,
    },
    ForUnsuback {
        id: PacketIdentifier,
    },
}

impl Waiting {
    fn is_waiting(&self) -> bool {
        match self {
            Self::None => false,
            Self::ForPuback { id: _ } => true,
            Self::ForSuback { id: _, qos: _ } => true,
            Self::ForUnsuback { id: _ } => true,
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
            Self::Idle => false,
            Self::Connecting(_) => true,
            Self::Connected(connection_data) => connection_data.waiting.is_waiting(),
            Self::Errored => false,
            Self::Disconnected => false,
        }
    }

    fn connect<const P: usize, const W: usize>(
        &mut self,
        connect: &Connect<'_, P, W>,
    ) -> Result<(), ClientStateError> {
        match self {
            ClientStateNoQueue::Idle => {
                *self = Self::Connecting(RequestedConnectionInfo {
                    clean_start: connect.clean_start(),
                    keep_alive: connect.keep_alive(),
                });
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

    fn publish_with_properties<'b, const P: usize>(
        &mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
        properties: Vec<PublishProperty<'b>, P>,
    ) -> Result<Publish<'b, P>, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionState { info: _, waiting }) => {
                let publish_packet_identifier = match qos {
                    QualityOfService::Qos0 => Ok(PublishPacketIdentifier::None),
                    QualityOfService::Qos1 if waiting.is_waiting() => {
                        Err(ClientStateError::ClientIsWaitingForResponse)
                    }
                    QualityOfService::Qos1 => Ok(PublishPacketIdentifier::Qos1(
                        Self::PUBLISH_PACKET_IDENTIFIER,
                    )),
                    QualityOfService::Qos2 => Err(ClientStateError::Qos2NotSupported),
                }?;

                let publish = Publish::new(
                    false,
                    retain,
                    topic_name,
                    publish_packet_identifier,
                    payload,
                    properties,
                );

                if qos == QualityOfService::Qos1 {
                    *waiting = Waiting::ForPuback {
                        id: Self::PUBLISH_PACKET_IDENTIFIER,
                    };
                }

                Ok(publish)
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn subscribe_packet<'b>(
        &mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<Subscribe<'b, 0, 0>, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionState { info: _, waiting }) => {
                if waiting.is_waiting() {
                    Err(ClientStateError::ClientIsWaitingForResponse)
                } else if maximum_qos == QualityOfService::Qos2 {
                    Err(ClientStateError::Qos2NotSupported)
                } else {
                    let first_request = SubscriptionRequest::new(topic_name, maximum_qos);
                    let subscribe: Subscribe<'_, 0, 0> = Subscribe::new(
                        Self::SUBSCRIBE_PACKET_IDENTIFIER,
                        first_request,
                        Vec::new(),
                        Vec::new(),
                    );
                    Ok(subscribe)
                }
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn subscribe_update<'b, const P: usize, const S: usize>(
        &mut self,
        packet: &Subscribe<'b, P, S>,
    ) -> Result<(), ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionState { info: _, waiting }) => {
                if waiting.is_waiting() {
                    Err(ClientStateError::ClientIsWaitingForResponse)
                } else if packet.request_count() > 1 {
                    Err(ClientStateError::MultipleSubscriptionRequestsNotSupported)
                } else if packet.request_maximum_qos() == QualityOfService::Qos2 {
                    Err(ClientStateError::Qos2NotSupported)
                } else {
                    *waiting = Waiting::ForSuback {
                        id: *packet.packet_identifier(),
                        qos: packet.request_maximum_qos(),
                    };

                    Ok(())
                }
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn unsubscribe_packet<'b>(
        &mut self,
        topic_name: &'b str,
    ) -> Result<Unsubscribe<'b, 0, 0>, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionState { info: _, waiting }) => {
                if waiting.is_waiting() {
                    Err(ClientStateError::ClientIsWaitingForResponse)
                } else {
                    let unsubscribe: Unsubscribe<'_, 0, 0> = Unsubscribe::new(
                        Self::UNSUBSCRIBE_PACKET_IDENTIFIER,
                        topic_name,
                        Vec::new(),
                        Vec::new(),
                    );

                    Ok(unsubscribe)
                }
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn unsubscribe_update<'b, const P: usize, const S: usize>(
        &mut self,
        packet: &Unsubscribe<'b, P, S>,
    ) -> Result<(), ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionState { info: _, waiting }) => {
                if waiting.is_waiting() {
                    Err(ClientStateError::ClientIsWaitingForResponse)
                } else {
                    *waiting = Waiting::ForUnsuback {
                        id: *packet.packet_identifier(),
                    };

                    Ok(())
                }
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn send_ping(&mut self) -> Result<Pingreq, ClientStateError> {
        match self {
            ClientStateNoQueue::Connected(ConnectionState { info, waiting: _ }) => {
                info.pending_ping_count += 1;
                Ok(Pingreq::default())
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn receive<'a, 'b, const P: usize, const W: usize, const S: usize>(
        &mut self,
        packet: PacketGeneric<'a, P, W, S>,
    ) -> Result<ClientStateReceiveEvent<'a, 'b, P>, ClientStateError> {
        match self {
            // If we are connecting, we only expect a Connack packet
            // (server cannot disconnect before Connack [MQTT-3.14.0-1])
            // or an Auth packet [MQTT-3.2.0-1]
            ClientStateNoQueue::Connecting(RequestedConnectionInfo {
                clean_start,
                keep_alive,
            }) => match packet {
                PacketGeneric::Connack(connack) => match connack.reason_code() {
                    ConnectReasonCode::Success => {
                        let session_present = connack.session_present();

                        // If there's a session, but we requested a clean start, this is an error
                        if session_present && *clean_start {
                            return Err(ClientStateError::UnexpectedSessionPresentForCleanStart);
                        }

                        // Keep alive is the one we requested, unless server returns a new one as a property
                        let mut actual_keep_alive = *keep_alive;
                        for p in connack.properties().iter() {
                            if let ConnackProperty::ServerKeepAlive(server_keep_alive) = p {
                                actual_keep_alive = server_keep_alive.value();
                            }
                        }

                        let info = ConnectionInfo {
                            pending_ping_count: 0,
                            session_present,
                            keep_alive: actual_keep_alive,
                        };

                        *self = Self::Connected(ConnectionState {
                            info,
                            waiting: Waiting::None,
                        });

                        Ok(ClientStateReceiveEvent::Ack)
                    }
                    reason_code => Err(ClientStateError::Connect(*reason_code)),
                },
                PacketGeneric::Auth(_) => Err(ClientStateError::AuthNotSupported),
                _ => Err(ClientStateError::ReceivedPacketOtherThanConnackOrAuthWhenConnecting),
            },

            // If we are connected, we handle all client packets other than Connack
            ClientStateNoQueue::Connected(ConnectionState { info, waiting }) => match packet {
                PacketGeneric::Publish(publish) => match publish.publish_packet_identifier() {
                    PublishPacketIdentifier::None => {
                        Ok(ClientStateReceiveEvent::Publish { publish })
                    }
                    PublishPacketIdentifier::Qos1(packet_identifier) => {
                        let puback =
                            Puback::new(*packet_identifier, PublishReasonCode::Success, Vec::new());
                        Ok(ClientStateReceiveEvent::PublishAndPuback { publish, puback })
                    }
                    PublishPacketIdentifier::Qos2(_) => {
                        Err(ClientStateError::ReceivedQos2PublishNotSupported)
                    }
                },

                PacketGeneric::Puback(puback) => {
                    let ack_id = puback.packet_identifier();
                    match waiting {
                        Waiting::ForPuback { id } if id == ack_id => {
                            *waiting = Waiting::None;

                            let reason_code = puback.reason_code();
                            if reason_code.is_error() {
                                Err(ClientStateError::Publish(*reason_code))
                            } else if reason_code == &PublishReasonCode::NoMatchingSubscribers {
                                Ok(ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers)
                            } else {
                                Ok(ClientStateReceiveEvent::Ack)
                            }
                        }
                        Waiting::ForPuback { id: _ } => {
                            Err(ClientStateError::UnexpectedPubackPacketIdentifier)
                        }
                        _ => Err(ClientStateError::UnexpectedPuback),
                    }
                }

                PacketGeneric::Suback(suback) => {
                    let ack_id = suback.packet_identifier();

                    match waiting {
                        Waiting::ForSuback { id, qos } if id == ack_id => {
                            let maximum_qos = *qos;
                            *waiting = Waiting::None;

                            let reason_code = suback.first_reason_code();
                            let granted_qos = match reason_code {
                                SubscribeReasonCode::Success => QualityOfService::Qos0,
                                SubscribeReasonCode::GrantedQos1 => QualityOfService::Qos1,
                                SubscribeReasonCode::GrantedQos2 => QualityOfService::Qos2,
                                err => return Err(ClientStateError::Subscribe(*err)),
                            };

                            if granted_qos != maximum_qos {
                                Ok(
                                    ClientStateReceiveEvent::SubscriptionGrantedBelowMaximumQos {
                                        granted_qos,
                                        maximum_qos,
                                    },
                                )
                            } else {
                                Ok(ClientStateReceiveEvent::Ack)
                            }
                        }
                        Waiting::ForSuback { id: _, qos: _ } => {
                            Err(ClientStateError::UnexpectedSubackPacketIdentifier)
                        }
                        _ => Err(ClientStateError::UnexpectedSuback),
                    }
                }
                PacketGeneric::Unsuback(unsuback) => {
                    let ack_id = unsuback.packet_identifier();

                    match waiting {
                        Waiting::ForUnsuback { id } if id == ack_id => {
                            *waiting = Waiting::None;

                            let reason_code = unsuback.first_reason_code();
                            if reason_code.is_error() {
                                Err(ClientStateError::Unsubscribe(*reason_code))
                            } else if reason_code == &UnsubscribeReasonCode::NoSubscriptionExisted {
                                Ok(ClientStateReceiveEvent::NoSubscriptionExisted)
                            } else {
                                Ok(ClientStateReceiveEvent::Ack)
                            }
                        }
                        Waiting::ForUnsuback { id: _ } => {
                            Err(ClientStateError::UnexpectedUnsubackPacketIdentifier)
                        }
                        _ => Err(ClientStateError::UnexpectedUnsuback),
                    }
                }
                PacketGeneric::Pingresp(_pingresp) => {
                    if info.pending_ping_count > 0 {
                        info.pending_ping_count -= 1;
                        Ok(ClientStateReceiveEvent::Ack)
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

    fn pending_ping_count(&self) -> u32 {
        match self {
            ClientStateNoQueue::Connected(connection_state) => {
                connection_state.info.pending_ping_count
            }
            _ => 0,
        }
    }
}
