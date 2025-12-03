use heapless::{FnvIndexMap, Vec};

use crate::{
    client_state::{ClientState, ClientStateError, ClientStateReceiveEvent},
    data::{
        packet_identifier::{PacketIdentifier, PublishPacketIdentifier},
        property::{ConnackProperty, Property, PublishProperty},
        quality_of_service::QualityOfService,
        reason_code::{
            ConnectReasonCode, PublishReasonCode, SubscribeReasonCode, UnsubscribeReasonCode,
        },
    },
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

pub struct RequestedConnectionInfo {
    clean_start: bool,
    keep_alive: u16,
}

pub struct ConnectionInfo {
    pending_ping_count: u32,
    pub session_present: bool,
    pub keep_alive: u16,
}

/// TODO: Set this with features
pub const Q: usize = 128;

// NOTE: Q must always be less than the max packet idenfier (which uses u16), so that we can't have
// all [`PacketIdentifiers`] in use. Since we always have at least one free, we can ensure we can
// always find the next id to use.
const _: () = {
    assert!(Q < u16::MAX as usize);
};

pub struct ConnectionState {
    info: ConnectionInfo,
    pending: FnvIndexMap<PacketIdentifier, Pending, Q>,
    next_id: PacketIdentifier,
}

impl ConnectionState {
    fn new(info: ConnectionInfo) -> Self {
        Self {
            info,
            pending: Default::default(),
            next_id: Default::default(),
        }
    }

    /// Check whether we have free space in pending
    fn has_free_pending(&self) -> bool {
        self.pending.len() < self.pending.capacity()
    }

    /// If we have free space in the queue, assign the next available id to the
    /// given `Pending` item
    fn add_pending(&mut self, pending: Pending) -> Result<PacketIdentifier, ClientStateError> {
        // We can only assign an id if we have space to track it
        if self.has_free_pending() {
            let id = self.next_id;
            self.pending
                .insert(id, pending)
                .expect("Error inserting pending even after checking for free space");

            // Find the next free id
            // Note that this loop will terminate since pending cannot contain all
            // packet identifiers, see assert on const Q
            loop {
                self.next_id.increment_wrapping();
                if !self.pending.contains_key(&self.next_id) {
                    break;
                }
            }

            Ok(id)
        } else {
            // Queue is full, so we need to wait for at least one (expected/valid) response
            Err(ClientStateError::ClientIsWaitingForResponse)
        }
    }

    fn publish(&mut self) -> Result<PacketIdentifier, ClientStateError> {
        self.add_pending(Pending::Puback)
    }

    fn subscribe(
        &mut self,
        maximum_qos: QualityOfService,
    ) -> Result<PacketIdentifier, ClientStateError> {
        self.add_pending(Pending::Suback { maximum_qos })
    }

    fn unsubscribe(&mut self) -> Result<PacketIdentifier, ClientStateError> {
        self.add_pending(Pending::Unsuback)
    }

    fn add_ping(&mut self) {
        self.info.pending_ping_count += 1;
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum Pending {
    Puback,
    Suback { maximum_qos: QualityOfService },
    Unsuback,
}

#[allow(clippy::large_enum_variant)]
pub enum ClientStateQueue {
    Idle,
    Connecting(RequestedConnectionInfo),
    Connected(ConnectionState),
    Errored,
    Disconnected,
}

impl ClientStateQueue {
    pub fn new() -> Self {
        Self::Idle
    }
}

impl Default for ClientStateQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientState for ClientStateQueue {
    fn waiting_for_responses(&self) -> bool {
        match self {
            Self::Idle => false,
            Self::Connecting(_) => true,
            Self::Connected(connection_data) => !connection_data.pending.is_empty(),
            Self::Errored => false,
            Self::Disconnected => false,
        }
    }

    fn connect<const P: usize, const W: usize>(
        &mut self,
        connect: &Connect<'_, P, W>,
    ) -> Result<(), ClientStateError> {
        match self {
            ClientStateQueue::Idle => {
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
            ClientStateQueue::Connected(_d) => {
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
            ClientStateQueue::Connected(state) => {
                let publish_packet_identifier = match qos {
                    QualityOfService::Qos0 => Ok(PublishPacketIdentifier::None),
                    QualityOfService::Qos1 => Ok(PublishPacketIdentifier::Qos1(state.publish()?)),
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
            ClientStateQueue::Connected(state) => {
                if maximum_qos == QualityOfService::Qos2 {
                    Err(ClientStateError::Qos2NotSupported)
                } else {
                    let first_request = SubscriptionRequest::new(topic_name, maximum_qos);
                    let subscribe: Subscribe<'_, 0, 0> = Subscribe::new(
                        state.subscribe(maximum_qos)?,
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

    fn unsubscribe<'b>(
        &mut self,
        topic_name: &'b str,
    ) -> Result<Unsubscribe<'b, 0, 0>, ClientStateError> {
        match self {
            ClientStateQueue::Connected(state) => {
                let unsubscribe: Unsubscribe<'_, 0, 0> =
                    Unsubscribe::new(state.unsubscribe()?, topic_name, Vec::new(), Vec::new());

                Ok(unsubscribe)
            }
            _ => Err(ClientStateError::NotConnected),
        }
    }

    fn send_ping(&mut self) -> Result<Pingreq, ClientStateError> {
        match self {
            ClientStateQueue::Connected(state) => {
                state.add_ping();
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
            ClientStateQueue::Connecting(RequestedConnectionInfo {
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

                        *self = Self::Connected(ConnectionState::new(info));

                        Ok(ClientStateReceiveEvent::Ack)
                    }
                    reason_code => Err(ClientStateError::Connect(*reason_code)),
                },
                PacketGeneric::Auth(_) => Err(ClientStateError::AuthNotSupported),
                _ => Err(ClientStateError::ReceivedPacketOtherThanConnackOrAuthWhenConnecting),
            },

            // If we are connected, we handle all client packets other than Connack
            ClientStateQueue::Connected(state) => match packet {
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
                    if state.pending.remove(ack_id) == Some(Pending::Puback) {
                        let reason_code = puback.reason_code();
                        if reason_code.is_error() {
                            Err(ClientStateError::Publish(*reason_code))
                        } else if reason_code == &PublishReasonCode::NoMatchingSubscribers {
                            Ok(ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers)
                        } else {
                            Ok(ClientStateReceiveEvent::Ack)
                        }
                    } else {
                        Err(ClientStateError::UnexpectedPubackPacketIdentifier)
                    }
                }

                PacketGeneric::Suback(suback) => {
                    let ack_id = suback.packet_identifier();

                    match state.pending.remove(ack_id) {
                        Some(Pending::Suback { maximum_qos }) => {
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
                        _ => Err(ClientStateError::UnexpectedSubackPacketIdentifier),
                    }
                }
                PacketGeneric::Unsuback(unsuback) => {
                    let ack_id = unsuback.packet_identifier();

                    if state.pending.remove(ack_id) == Some(Pending::Unsuback) {
                        let reason_code = unsuback.first_reason_code();
                        if reason_code.is_error() {
                            Err(ClientStateError::Unsubscribe(*reason_code))
                        } else if reason_code == &UnsubscribeReasonCode::NoSubscriptionExisted {
                            Ok(ClientStateReceiveEvent::NoSubscriptionExisted)
                        } else {
                            Ok(ClientStateReceiveEvent::Ack)
                        }
                    } else {
                        Err(ClientStateError::UnexpectedUnsubackPacketIdentifier)
                    }
                }
                PacketGeneric::Pingresp(_pingresp) => {
                    if state.info.pending_ping_count > 0 {
                        state.info.pending_ping_count -= 1;
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
}
