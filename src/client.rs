use crate::{
    client_state::{ClientState, ClientStateError, ClientStateNoQueue, ClientStateReceiveEvent},
    data::{quality_of_service::QualityOfService, reason_code::DisconnectReasonCode},
    error::{PacketReadError, PacketWriteError},
    packet_client::{Connection, PacketClient},
    packets::{connect::Connect, packet_generic::PacketGeneric},
};

/// [Client] error
#[derive(Debug, PartialEq)]
pub enum ClientError {
    PacketWrite(PacketWriteError),
    PacketRead(PacketReadError),
    ClientState(ClientStateError),
    TimeoutOnResponsePacket,
    Disconnected(DisconnectReasonCode),
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

    /// Subscribe to a topic by name
    async fn subscribe_to_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: &QualityOfService,
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
    F: Fn(&str, &[u8]) -> Result<(), ClientError>,
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
    F: Fn(&str, &[u8]) -> Result<(), ClientError>,
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
        let mut waiting = true;
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

    // pub async fn send<P>(&mut self, packet: P) -> Result<(), ClientError>
    // where
    //     P: Packet + write::Write,
    // {
    //     match self.packet_client.send(packet).await {
    //         Ok(()) => {
    //             self.wait_for_responses(self.timeout_millis).await?;
    //             Ok(())
    //         }
    //         Err(e) => {
    //             self.client_state.error();
    //             Err(e.into())
    //         }
    //     }
    // }
}

impl<'a, C, D, F> Client<'a> for ClientNoQueue<'a, C, D, F>
where
    C: Connection,
    D: Delay,
    F: Fn(&str, &[u8]) -> Result<(), ClientError>,
{
    async fn connect<const PROPERTIES_N: usize>(
        &mut self,
        packet: Connect<'_, PROPERTIES_N>,
    ) -> Result<(), ClientError> {
        self.client_state.connect(&packet)?;
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

    async fn disconnect(&mut self) -> Result<(), ClientError> {
        let packet = self.client_state.disconnect()?;
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

    async fn send_message<'b>(
        &'b mut self,
        topic_name: &'b str,
        message: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError> {
        let packet = self
            .client_state
            .send_message(topic_name, message, qos, retain)?;
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

    async fn subscribe_to_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: &QualityOfService,
    ) -> Result<(), ClientError> {
        let packet = self
            .client_state
            .subscribe_to_topic(topic_name, maximum_qos)?;
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

    async fn unsubscribe_from_topic<'b>(
        &'b mut self,
        topic_name: &'b str,
    ) -> Result<(), ClientError> {
        let packet = self.client_state.unsubscribe_from_topic(topic_name)?;
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

    async fn send_ping(&mut self) -> Result<(), ClientError> {
        let packet = self.client_state.send_ping()?;
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

    async fn poll(&mut self, wait: bool) -> Result<bool, ClientError> {
        let to_send = {
            let packet: Option<PacketGeneric<'_, 16, 16>> = if wait {
                Some(self.packet_client.receive().await?)
            } else {
                self.packet_client.receive_if_ready().await?
            };

            if packet.is_none() {
                return Ok(false);
            }

            let packet = packet.unwrap();

            let event = self.client_state.receive(packet)?;

            match event {
                ClientStateReceiveEvent::None => None,
                ClientStateReceiveEvent::Publish { publish } => {
                    (self.message_handler)(publish.topic_name(), publish.payload())?;
                    None
                }
                ClientStateReceiveEvent::PublishAndPubAck { publish, puback } => {
                    (self.message_handler)(publish.topic_name(), publish.payload())?;
                    Some(puback)
                }
                ClientStateReceiveEvent::SubscriptionGrantedBelowMaximumQoS {
                    granted_qos: _,
                    maximum_qos: _,
                } => None,
                ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers => None,

                // TODO: Include disconnect reason
                ClientStateReceiveEvent::Disconnect { disconnect } => {
                    return Err(ClientError::Disconnected(*disconnect.reason_code()));
                }
            }
        };

        if let Some(packet) = to_send {
            self.packet_client.send(packet).await?;
        }

        Ok(true)
    }
}
