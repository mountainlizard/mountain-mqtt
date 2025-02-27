use core::fmt::{Display, Formatter};

use crate::{
    client_state::{ClientState, ClientStateError, ClientStateNoQueue, ClientStateReceiveEvent},
    codec::write,
    data::{quality_of_service::QualityOfService, reason_code::DisconnectReasonCode},
    error::{PacketReadError, PacketWriteError},
    packet_client::{Connection, PacketClient},
    packets::{connect::Connect, packet::Packet, packet_generic::PacketGeneric},
};

/// [Client] error
#[derive(Debug, PartialEq)]
pub enum ClientError {
    PacketWrite(PacketWriteError),
    PacketRead(PacketReadError),
    ClientState(ClientStateError),
    TimeoutOnResponsePacket,
    Disconnected(DisconnectReasonCode),
    MessageHandlerError,
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

impl Display for ClientError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PacketWrite(e) => write!(f, "PacketWrite({})", e),
            Self::PacketRead(e) => write!(f, "PacketRead({})", e),
            Self::ClientState(e) => write!(f, "ClientState({})", e),
            Self::TimeoutOnResponsePacket => write!(f, "TimeoutOnResponsePacket"),
            Self::Disconnected(e) => write!(f, "Disconnected({})", e),
            Self::MessageHandlerError => write!(f, "MessageHandlerError"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Message<'a> {
    pub topic_name: &'a str,
    pub payload: &'a [u8],
}

/// A simple client interface for connecting to an MQTT server
#[allow(async_fn_in_trait)]
pub trait Client<'a> {
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
    /// This updates the state of the client, and calls the message_handler if
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

    /// Publish a message with given payload to a given topic
    async fn publish<'b>(
        &'b mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError>;
}

#[allow(async_fn_in_trait)]
pub trait Delay {
    /// Pauses execution for at minimum `us` microseconds. Pause can be longer
    /// if the implementation requires it due to precision/timing issues.
    async fn delay_us(&mut self, us: u32);
}

pub struct ClientNoQueue<'a, C, D, F>
where
    C: Connection,
    D: Delay,
    F: Fn(Message) -> Result<(), ClientError>,
{
    packet_client: PacketClient<'a, C>,
    client_state: ClientStateNoQueue,
    delay: D,
    timeout_millis: u32,
    message_handler: F,
}

impl<'a, C, D, F> ClientNoQueue<'a, C, D, F>
where
    C: Connection,
    D: Delay,
    F: Fn(Message) -> Result<(), ClientError>,
{
    pub fn new(
        connection: C,
        buf: &'a mut [u8],
        delay: D,
        timeout_millis: u32,
        message_handler: F,
    ) -> Self {
        let packet_client = PacketClient::new(connection, buf);
        let client_state = ClientStateNoQueue::default();
        Self {
            packet_client,
            client_state,
            delay,
            timeout_millis,
            message_handler,
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

    pub async fn send_wait_for_responses<P>(&mut self, packet: P) -> Result<(), ClientError>
    where
        P: Packet + write::Write,
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

    pub async fn send<P>(&mut self, packet: P) -> Result<(), ClientError>
    where
        P: Packet + write::Write,
    {
        let r = self.packet_client.send(packet).await;
        if r.is_err() {
            self.client_state.error();
        }
        r?;
        Ok(())
    }
}

impl<'a, C, D, F> Client<'a> for ClientNoQueue<'a, C, D, F>
where
    C: Connection,
    D: Delay,
    F: Fn(Message) -> Result<(), ClientError>,
{
    async fn connect<const PROPERTIES_N: usize>(
        &mut self,
        packet: Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientError> {
        self.client_state.connect(&packet)?;
        self.send_wait_for_responses(packet).await
    }

    async fn disconnect(&mut self) -> Result<(), ClientError> {
        let packet = self.client_state.disconnect()?;
        self.send(packet).await
    }

    async fn publish<'b>(
        &'b mut self,
        topic_name: &'b str,
        message: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError> {
        let packet = self
            .client_state
            .publish(topic_name, message, qos, retain)?;
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
            let packet: Option<PacketGeneric<'_, 16, 16>> = if wait {
                Some(self.packet_client.receive().await?)
            } else {
                self.packet_client.receive_if_ready().await?
            };

            if let Some(packet) = packet {
                let event = self.client_state.receive(packet)?;

                match event {
                    ClientStateReceiveEvent::None => None,
                    ClientStateReceiveEvent::Publish { publish } => {
                        (self.message_handler)(Message {
                            topic_name: publish.topic_name(),
                            payload: publish.payload(),
                        })?;
                        None
                    }
                    ClientStateReceiveEvent::PublishAndPubAck { publish, puback } => {
                        (self.message_handler)(Message {
                            topic_name: publish.topic_name(),
                            payload: publish.payload(),
                        })?;
                        Some(puback)
                    }

                    // Not an error, no handler for now
                    ClientStateReceiveEvent::SubscriptionGrantedBelowMaximumQoS {
                        granted_qos: _,
                        maximum_qos: _,
                    } => None,

                    // Not an error, no handler for now
                    ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers => None,

                    // Not an error, no handler for now
                    ClientStateReceiveEvent::NoSubscriptionExisted => None,

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
