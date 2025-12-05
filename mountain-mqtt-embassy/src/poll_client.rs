use core::{future, net::Ipv4Addr};

use crate::{
    packet_bin::{self, PacketBin},
    packet_bin_client::PacketBinClient,
};
use defmt::{info, warn};
use embassy_futures::select::{select3, Either3};
use embassy_net::{
    tcp::{ConnectError, TcpSocket},
    Stack,
};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::{Duration, Instant, Timer};
use embedded_io_async::Write;
use heapless::Vec;
use mountain_mqtt::{
    client::{ClientError, ClientReceivedEvent, ConnectionSettings},
    client_state::{ClientState, ClientStateError, ClientStateReceiveEvent},
    data::{
        packet_type::PacketType,
        property::{ConnectProperty, PublishProperty},
        quality_of_service::QualityOfService,
    },
    error::PacketWriteError,
    packets::{
        connect::{Connect, Will},
        disconnect::Disconnect,
        packet::Packet,
        packet_generic::PacketGeneric,
        pingreq::Pingreq,
    },
};

#[derive(Debug, Copy, Clone)]
/// Settings for a [`PollClient`]
pub struct Settings {
    /// The address of the MQTT server
    pub address: Ipv4Addr,

    /// The port of the MQTT server
    pub port: u16,

    /// The maximum time between packets received from the server,
    /// before we consider it unresponsive, leading to a
    /// [`ClientError::ReceiveTimeoutServerUnresponsive`]
    receive_timeout: Duration,

    /// The time between pings sent to the server to keep the
    /// connection alive
    ping_interval: Duration,

    /// We will only send one ping at a time - i.e. we need to see
    /// a response to a sent ping request before we will send another
    /// request.
    /// If we reach the time when a ping is due, and the previous one
    /// has not had a response, we will delay this additional time before
    /// trying again.
    ping_retry_delay: Duration,
}

impl Settings {
    pub fn new(address: Ipv4Addr, port: u16) -> Self {
        Self {
            address,
            port,
            receive_timeout: Duration::from_secs(10),
            ping_interval: Duration::from_secs(2),
            ping_retry_delay: Duration::from_millis(100),
        }
    }
}

pub enum MqttConnectionError {
    ConnectError(ConnectError),
    ClientError(ClientError),
    TcpWriteError(embassy_net::tcp::Error),
}

#[cfg(feature = "defmt")]
impl defmt::Format for MqttConnectionError {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::ConnectError(e) => defmt::write!(f, "ConnectError({})", e),
            Self::ClientError(e) => defmt::write!(f, "ClientError({})", e),
            Self::TcpWriteError(e) => defmt::write!(f, "TcpWriteError({})", e),
        }
    }
}

impl From<ConnectError> for MqttConnectionError {
    fn from(value: ConnectError) -> Self {
        MqttConnectionError::ConnectError(value)
    }
}

impl From<ClientError> for MqttConnectionError {
    fn from(value: ClientError) -> Self {
        MqttConnectionError::ClientError(value)
    }
}

pub async fn run_mqtt_connection<S, M, const N: usize, const P: usize>(
    settings: Settings,
    stack: Stack<'static>,
    f: impl AsyncFnOnce(&mut PollClient<S, M, N, P>) -> Result<(), ClientError>,
) -> Result<(), MqttConnectionError>
where
    M: RawMutex,
    S: ClientState + Default,
{
    let mut rx_buffer = [0; N];
    let mut tx_buffer = [0; N];

    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);

    socket.set_timeout(None);

    let remote_endpoint = (settings.address, settings.port);
    info!("MQTT socket connecting to {:?}...", remote_endpoint);
    socket.connect(remote_endpoint).await?;
    info!("MQTT socket connected!");

    let rx_channel: Channel<M, PacketBin<N>, 1> = Channel::new();
    let rx_channel_sender = rx_channel.sender();

    let tx_channel: Channel<M, PacketBin<N>, 1> = Channel::new();
    let tx_channel_receiver = tx_channel.receiver();

    let (mut rx, mut tx) = socket.split();

    let rx_fut = async {
        loop {
            match packet_bin::receive_packet_bin(&mut rx).await {
                Ok(packet_bin) => rx_channel_sender.send(packet_bin).await,
                Err(e) => return e,
            }
        }
    };

    let tx_fut = async {
        loop {
            let write = tx_channel_receiver.receive().await;
            // Ignore packets with length 0 - we can use these as a way to flush
            // the buffer.
            if write.len > 0 {
                if let Err(e) = tx.write_all(write.msg_data()).await {
                    return e;
                }
            }
        }
    };

    let mut client = PollClient::new(
        tx_channel.sender(),
        rx_channel.receiver(),
        settings,
        S::default(),
    );

    match select3(rx_fut, tx_fut, f(&mut client)).await {
        Either3::First(e) => {
            warn!("Finished network comms with read error {:?}", e);
            Err(e)?
        }
        Either3::Second(e) => {
            warn!("Finished network comms with write error {:?}", e);
            Err(MqttConnectionError::ClientError(ClientError::PacketWrite(
                PacketWriteError::ConnectionSend,
            )))
        }
        Either3::Third(r) => {
            info!("Finished network comms by polling completing with {:?}", r);
            r?;
            Ok(())
        }
    }
}

/// An MQTT client that works by regularly polling for new received messages,
/// rather than using a stream of events.
pub struct PollClient<'a, S, M, const N: usize, const P: usize>
where
    M: RawMutex,
    S: ClientState,
{
    /// Used to track the client state, e.g. whether we are connected,
    /// whether there are pending acks, etc.
    client_state: S,

    /// Used to send and receive [`PacketBin`] instances, each containing
    /// and MQTT packet in binary format.
    raw_client: PacketBinClient<'a, M, N>,

    /// The end of the timeout for received packets (specifically connack and pingresp)
    /// from the server.
    /// This is initially None. It is initialised when a connection is started (by
    /// sending a connect packet), and then updated whenever a  relevant packet is
    /// received.
    /// If we ever reach this timeout instant, then the server has not
    /// replied for too long, and is unresponsive leading to an error being reported.
    /// Note that the receive timeout is only active when this is Some.
    receive_timeout_at: Option<Instant>,

    /// The scheduled time for sending the next ping.
    /// This is initially None. It is initialised when a connection is
    /// made, and then updated whenever we attempt to send a ping. Note that
    /// we only send one ping at a time, if ping_time occurs while there is still
    /// a ping that has not been responded to, we will delay before attempting
    /// to send another ping.
    /// Note that pings are only active when this is Some.
    ping_at: Option<Instant>,

    /// When we sent the [`Connect`] packet to start connection
    connection_start: Option<Instant>,

    /// Client settings
    settings: Settings,
}

/// Implements a relatively low-level but flexible client that is operated
/// based on regularly polling for new messages.
impl<'a, S, M, const N: usize, const P: usize> PollClient<'a, S, M, N, P>
where
    M: RawMutex,
    S: ClientState,
{
    /// Create a PollClient using [`Sender`] and [`Receiver`] to
    /// send/receive MQTT packets as [`PacketBin`].
    /// The sender and receiver must be already connected at the TCP/IP layer,
    /// with no data yet sent or received.
    pub fn new(
        sender: Sender<'a, M, PacketBin<N>, 1>,
        receiver: Receiver<'a, M, PacketBin<N>, 1>,
        settings: Settings,
        client_state: S,
    ) -> Self {
        Self {
            receive_timeout_at: None,
            ping_at: None,
            connection_start: None,
            client_state,
            raw_client: PacketBinClient::new(sender, receiver),
            settings,
        }
    }

    /// Connect to the server with [`ConnectionSettings`], see
    /// [`PollClient::connect_with_packet`] for details.
    /// NOT CANCEL-SAFE
    pub async fn connect(&mut self, settings: &ConnectionSettings<'_>) -> Result<(), ClientError> {
        self.connect_with_will::<0>(settings, None).await
    }

    /// Connect to the server with [`ConnectionSettings`], and an optional [`Will`],
    /// see [`PollClient::connect_with_packet`] for details.
    /// NOT CANCEL-SAFE
    pub async fn connect_with_will<const W: usize>(
        &mut self,
        settings: &ConnectionSettings<'_>,
        will: Option<Will<'_, W>>,
    ) -> Result<(), ClientError> {
        let mut properties = Vec::new();
        // By setting maximum topic alias to 0, we prevent the server
        // trying to use aliases, which we don't support. They are optional
        // and only provide for reduced packet size, but would require storing
        // topic names from the server for the length of the connection,
        // which might be awkward without alloc.
        properties
            .push(ConnectProperty::TopicAliasMaximum(0.into()))
            .unwrap();
        let packet: Connect<'_, 1, W> = Connect::new(
            settings.keep_alive(),
            *settings.username(),
            *settings.password(),
            settings.client_id(),
            true,
            will,
            properties,
        );

        self.connect_with_packet(packet).await
    }

    /// Connect to the server - this sends a [`Connect`] packet and  waits for
    /// the connection to be acknowledged, it will time out if the server is unresponsive
    /// NOT CANCEL-SAFE
    pub async fn connect_with_packet<const PP: usize, const W: usize>(
        &mut self,
        packet: Connect<'_, PP, W>,
    ) -> Result<(), ClientError> {
        self.client_state.connect(&packet)?;
        self.raw_client.send_packet(&packet).await?;

        // Sending packet is the start of our connection
        self.connection_start = Some(Instant::now());

        // We are expecting a server reply (the connack), so we can start the receive timeout
        // We don't start the ping interval yet since we shouldn't ping until
        // we are connected
        self.receive_timeout_at = Some(Instant::now() + self.settings.receive_timeout);

        // We now just wait for an ack
        self.wait_for_connected().await?;

        Ok(())
    }

    async fn wait_for_connected(&mut self) -> Result<(), ClientError> {
        while self.client_state.waiting_for_responses() {
            // TODO: Use receive_bin when this supports receive_timeout (and ping interval,
            // although when used for connect this will not trigger since the ping interval
            // start won't have been set)
            // let packet_bin = self.raw_client.receive_bin().await;
            let packet_bin = self.receive().await?;
            let packet: mountain_mqtt::packets::packet_generic::PacketGeneric<'_, P, 0, 0> =
                packet_bin.as_packet_generic()?;
            let event = self.client_state.receive(packet)?;
            match event {
                ClientStateReceiveEvent::Ack => {
                    // We should now start sending pings - start from when we started connection,
                    // since this is the last time we sent a packet
                    self.ping_at = self
                        .connection_start
                        .map(|s| s + self.settings.ping_interval);
                    info!("Client connected");
                }
                _ => {
                    return Err(ClientError::ClientState(
                        ClientStateError::ReceivedPacketOtherThanConnackOrAuthWhenConnecting,
                    ))
                }
            }
        }

        Ok(())
    }

    /// Send a ping - does not check whether one is needed, but will update the next ping time
    /// Cancel-safe: If a ping is sent, the client state is only updated (sync) after this succeeds
    async fn ping(&mut self) -> Result<(), ClientError> {
        info!("Maybe pinging...");
        if self.client_state.pending_ping_count() > 0 {
            info!("...Ping pending, will delay and retry");
            self.ping_at = Some(Instant::now() + self.settings.ping_retry_delay);
        } else {
            info!("...Pinging");
            // CANCEL-SAFETY: We need to send the ping first, then
            // if this completes we update the client_state and interval as sync operations.
            // This does involve just ignoring the
            // `Pingreq` packet we get back from the client state.
            // Either this async fn is dropped at the await point, and since
            // `send` is client safe, no packet is sent, OR we are not dropped,
            // the packet is sent, and we update the client state and interval.
            // Note that the packet is not actually sent immediately, it is just queued,
            // but if the actual sending fails then the client will error and should not
            // be used further.
            self.raw_client.send_packet(&Pingreq::default()).await?;
            self.client_state.send_ping()?;
            self.ping_at = Some(Instant::now() + self.settings.ping_interval);
        }
        Ok(())
    }

    /// Send a ping if more than the ping interval has elapsed (see [`Settings`]),
    /// and reset the ping interval if one was sent.
    /// Returns true if a ping was sent.
    /// Cancel-safe: Just calls through to cancel-safe [`Self::ping`] if needed.
    pub async fn ping_if_needed(&mut self) -> Result<bool, ClientError> {
        if let Some(ping_at) = self.ping_at {
            if ping_at < Instant::now() {
                self.ping().await?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn check_receive_timeout(&self) -> Result<(), ClientError> {
        if let Some(receive_timeout_at) = self.receive_timeout_at {
            if receive_timeout_at < Instant::now() {
                return Err(ClientError::ReceiveTimeoutServerUnresponsive);
            }
        }
        Ok(())
    }

    fn reset_receive_timeout(&mut self) {
        self.receive_timeout_at = Some(Instant::now() + self.settings.receive_timeout);
    }

    /// Check whether a new [`PacketBin`] is available immediately, and if so
    /// return it.
    /// Note that this method is still async - it will not await new data from the server,
    /// but it may need to perform other async operations, e.g. sending a ping. However these
    /// operations will either complete or result in an error in a bounded time (e.g. if data
    /// cannot be sent, the server will disconnect us for lack of response, and/or we will
    /// be unable to ping the server and so the client will disconnect since the server will
    /// appear unresponsive).
    /// This will handle sending pings as needed, and check if the server is unresponsive.
    /// Cancel-safe
    pub async fn try_receive(&mut self) -> Result<Option<PacketBin<N>>, ClientError> {
        self.check_receive_timeout()?;
        self.ping_if_needed().await?;

        // Cancel-safety: This comes last, so if await above is interrupted, we don't
        // receive a packet and then drop it. try_receive is sync and so can't be interrupted.
        let packet = self.raw_client.try_receive();
        Ok(packet)
    }

    /// Disconnect the client with default reason code (success) and no properties
    /// NOT CANCEL-SAFE
    pub async fn disconnect(&mut self) -> Result<(), ClientError> {
        self.disconnect_with_packet(Disconnect::default()).await
    }

    /// Disconnect the client with a provided packet (e.g. for reason code)
    /// NOT CANCEL-SAFE
    pub async fn disconnect_with_packet<'b, const PP: usize>(
        &mut self,
        packet: Disconnect<'b, PP>,
    ) -> Result<(), ClientError> {
        self.raw_client.send_packet(&packet).await?;
        // Send an empty packet to flush send queue so we know the
        // disconnect packet has actually made it to the network
        self.raw_client.send(PacketBin::empty()).await;
        self.client_state.disconnect()?;
        Ok(())
    }

    async fn wait_for_time(time: Option<Instant>) {
        if let Some(time) = time {
            Timer::at(time).await
        } else {
            // When time is None, check is not enabled, so never complete
            future::pending().await
        }
    }

    /// Wait to receive a new [`PacketBin`].
    /// This will handle sending pings as needed, and will also detect if the server is
    /// unresponsive, and will then return an error.
    ///
    /// Cancel-safe: This will leave the client in a valid state even if dropped, and
    /// will not lose packets if dropped. Therefore this can be used in a select. For
    /// example you may wish to select between [`PollClient::receive_bin`], and having
    /// an outgoing message you wish to publish, for example by receiving one on a channel
    /// from the rest of your application.
    pub async fn receive(&mut self) -> Result<PacketBin<N>, ClientError> {
        // Loop until we error or return a packet
        loop {
            // We need to deal with the next event - this is either the ping interval
            // elapsing, a receive timeout, or a packet arriving.
            // Note that all of these operations are cancel-safe
            // Note that even though we also check for receive timeouts in
            // `process`, we want to stop waiting for a packet immediately if this timeout occurs,
            // rather than waiting until we receive and process it.
            let r = select3(
                Self::wait_for_time(self.ping_at),
                Self::wait_for_time(self.receive_timeout_at),
                self.raw_client.receive(),
            )
            .await;

            match r {
                // Note that ping is cancel-safe
                Either3::First(()) => self.ping().await?,
                Either3::Second(()) => return Err(ClientError::ReceiveTimeoutServerUnresponsive),
                Either3::Third(packet_bin) => return Ok(packet_bin),
            };
        }
    }

    /// Request a subscription.
    /// This may require a response from the server, so after calling this, you must receive messages until
    /// [`PollClient::waiting_for_responses`] returns false, before calling any other methods that may
    /// require a response from the server.
    /// Cancel-safe: Unless subscribe packet is sent, client state won't be updated
    pub async fn subscribe(
        &mut self,
        topic_name: &str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError> {
        let packet = self
            .client_state
            .subscribe_packet(topic_name, maximum_qos)?;
        self.raw_client.send_packet(&packet).await?;
        self.client_state.subscribe_update(&packet)?;
        Ok(())
    }

    /// Request to unsubscribe from a topic
    /// This may require a response from the server, so after calling this, you must receive messages until
    /// [`PollClient::waiting_for_responses`] returns false, before calling any other methods that may
    /// require a response from the server.
    /// Cancel-safe: Unless unsubscribe packet is sent, client state won't be updated
    pub async fn unsubscribe(&mut self, topic_name: &str) -> Result<(), ClientError> {
        let packet = self.client_state.unsubscribe_packet(topic_name)?;
        self.raw_client.send_packet(&packet).await?;
        self.client_state.unsubscribe_update(&packet)?;
        Ok(())
    }

    /// True if client is waiting for a response from the server - if this is true, then you must receive and
    /// handle packets until it becomes false, before attempting to send any more packets.
    /// This is done by calling [`PollClient::receive`] or [`PollClient::try_receive`]
    /// and then passing any resulting packet to [`PollClient::process`], and
    /// handling any resulting [`ClientReceivedEvent`]s.
    pub fn waiting_for_responses(&self) -> bool {
        self.client_state.waiting_for_responses()
    }

    /// Publish a message with given payload to a given topic, with no properties
    /// This may require a response from the server, so after calling this, you must receive messages until
    /// [`PollClient::waiting_for_responses`] returns false, before calling any other methods that may
    /// require a response from the server.
    /// NOT CANCEL-SAFE
    pub async fn publish<'b>(
        &'b mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
    ) -> Result<(), ClientError> {
        self.publish_with_properties::<0>(topic_name, payload, qos, retain, Vec::new())
            .await
    }

    /// Publish a message with given payload to a given topic, with given properties
    /// This may require a response from the server, so after calling this, you must receive messages until
    /// [`PollClient::waiting_for_responses`] returns false, before calling any other methods that may
    /// require a response from the server.
    /// NOT CANCEL-SAFE
    pub async fn publish_with_properties<'b, const PP: usize>(
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
        self.raw_client.send_packet(&packet).await
    }

    /// Handle a [`PacketBin`], parsing it as a [`PacketGeneric`], then updating client state,
    /// sending any required response packet, and finally returning any [`ClientReceivedEvent`]
    /// resulting from the packet.
    /// This be called exactly once with each received [`PacketBin`]
    /// This also handles checking for receive timeouts, so it's best to call immediately after
    /// receiving each packet.
    /// NOT CANCEL-SAFE: It will update the client state before sending any required response
    /// packet, if the sending of the packet is cancelled then the client/connection state
    /// will be invalid. TODO: It's possible this could be made cancel-safe by splitting the
    /// client state handling into two parts - the first would produce a response packet but not
    /// change the state itself, and then when this packet had been sent successfully the client
    /// state would be updated by the second part of the handling.
    pub async fn process<'b>(
        &mut self,
        packet_bin: &'b PacketBin<N>,
    ) -> Result<ClientReceivedEvent<'b, P>, ClientError> {
        let (event, to_send) = {
            let packet: PacketGeneric<'_, P, 0, 0> = packet_bin.as_packet_generic()?;

            // We reset the receive timeout on connack and pingresp only.
            // We use the pings as our means of detecting any issue with packets getting
            // through on either outgoing or incoming channels. If we included other packet
            // types then even if pings were not being sent outgoing, we might not timeout
            // since the server could still be sending publish packets regularly.
            // Connack is safe to use since we'll only ever get one in response to our connect
            // packet, which essentially acts as our first ping.
            // We check here rather than on receive because we need to have decoded to
            // know the packet type
            match packet.packet_type() {
                PacketType::Connack | PacketType::Pingresp => self.reset_receive_timeout(),
                _ => {}
            }

            self.check_receive_timeout()?;

            let event = self.client_state.receive(packet)?;

            match event {
                ClientStateReceiveEvent::Ack => (ClientReceivedEvent::Ack, None),

                ClientStateReceiveEvent::Publish { publish } => {
                    if publish.topic_name().is_empty() {
                        return Err(ClientError::EmptyTopicNameWithAliasesDisabled);
                    }
                    (publish.into(), None)
                }

                ClientStateReceiveEvent::PublishAndPuback { publish, puback } => {
                    if publish.topic_name().is_empty() {
                        return Err(ClientError::EmptyTopicNameWithAliasesDisabled);
                    }
                    (publish.into(), Some(puback))
                }

                ClientStateReceiveEvent::SubscriptionGrantedBelowMaximumQos {
                    granted_qos,
                    maximum_qos,
                } => (
                    ClientReceivedEvent::SubscriptionGrantedBelowMaximumQos {
                        granted_qos,
                        maximum_qos,
                    },
                    None,
                ),

                ClientStateReceiveEvent::PublishedMessageHadNoMatchingSubscribers => (
                    ClientReceivedEvent::PublishedMessageHadNoMatchingSubscribers,
                    None,
                ),

                ClientStateReceiveEvent::NoSubscriptionExisted => {
                    (ClientReceivedEvent::NoSubscriptionExisted, None)
                }

                ClientStateReceiveEvent::Disconnect { disconnect } => {
                    return Err(ClientError::Disconnected(*disconnect.reason_code()));
                }
            }
        };

        // Send any resulting packet, no need to wait for responses
        if let Some(packet) = to_send {
            self.raw_client.send_packet(&packet).await?;
        }

        Ok(event)
    }
}
