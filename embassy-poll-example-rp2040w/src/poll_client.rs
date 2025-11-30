use core::future;

use crate::{packet_bin::PacketBin, raw_client::RawClient};
use defmt::info;
use embassy_futures::select::{select3, Either3};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::{Receiver, Sender},
};
use embassy_time::{Duration, Instant, Timer};
use heapless::Vec;
use mountain_mqtt::{
    client::{ClientError, ClientReceivedEvent, ConnectionSettings},
    client_state::{ClientState, ClientStateError, ClientStateNoQueue, ClientStateReceiveEvent},
    data::{
        property::{ConnectProperty, PublishProperty},
        quality_of_service::QualityOfService,
    },
    packets::{
        connect::{Connect, Will},
        packet_generic::PacketGeneric,
        pingreq::Pingreq,
    },
};

/// Settings for a [`PollClient`]
pub struct Settings {
    receive_timeout: Duration,
    ping_interval: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            receive_timeout: Duration::from_secs(10),
            ping_interval: Duration::from_secs(2),
        }
    }
}

pub struct PollClient<'a, M, const N: usize, const P: usize>
where
    M: RawMutex,
{
    /// Used to track the client state, e.g. whether we are connected,
    /// whether there are pending acks, etc.
    client_state: ClientStateNoQueue,

    /// Used to send and receive [`PacketBin`] instances, each containing
    /// and MQTT packet in binary format.
    raw_client: RawClient<'a, M, N>,

    /// The start of the timeout for received packets from the server.
    /// This is initially None. It is initialised when a
    /// connection is made, and then updated whenever a packet is
    /// received.
    /// If this is ever more than the receive_timeout in the past,
    /// then the server has not replied for too long, and is
    /// unresponsive leading to a disconnection.
    /// Note that the receive timeout is only active when this is Some.
    receive_timeout_start: Option<Instant>,

    /// The start of the interval for sending a ping.
    /// This is initially None. It is initialised when a connection is
    /// made, and then updated whenever a packet is sent.
    /// If this is ever more than the ping_interval in the past, then
    /// this client has not sent a packet for too long, and should
    /// immediately send a ping request to avoid the server
    /// disconnecting us as unresponsive.
    /// Note that pings are only active when this is Some.
    ping_interval_start: Option<Instant>,

    /// When we sent the [`Connect`] packet to start connection
    connection_start: Option<Instant>,

    /// Client settings
    settings: Settings,
}

/// Implements a relatively low-level but flexible client that is operated
/// based on regularly polling for new messages.
impl<'a, M, const N: usize, const P: usize> PollClient<'a, M, N, P>
where
    M: RawMutex,
{
    /// Create a PollClient using [`Sender`] and [`Receiver`] to
    /// send/receive MQTT packets as [`PacketBin`].
    /// The sender and receiver must be already connected at the TCP/IP layer,
    /// with no data yet sent or received.
    pub fn new(
        sender: Sender<'a, M, PacketBin<N>, 1>,
        receiver: Receiver<'a, M, PacketBin<N>, 1>,
        settings: Settings,
    ) -> Self {
        Self {
            receive_timeout_start: None,
            ping_interval_start: None,
            connection_start: None,
            client_state: ClientStateNoQueue::default(),
            raw_client: RawClient::new(sender, receiver),
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
        self.raw_client.send(packet).await?;

        // Sending packet is the start of our connection
        self.connection_start = Some(Instant::now());

        // We are expecting a server reply, so we can start the receive timeout
        // We don't start the ping interval yet since we shouldn't ping until
        // we are connected
        self.receive_timeout_start = Some(Instant::now());

        // All that can happen after first connecting is that we receive an Ack,
        // indicating we are connected and can continue, or
        self.wait_for_ack_only().await?;

        Ok(())
    }

    async fn wait_for_ack_only(&mut self) -> Result<(), ClientError> {
        while self.client_state.waiting_for_responses() {
            // TODO: Use receive_bin when this supports receive_timeout (and ping interval,
            // although when used for connect this will not trigger since the ping interval
            // start won't have been set)
            // let packet_bin = self.raw_client.receive_bin().await;
            let packet_bin = self.receive_bin().await?;
            // if let Some(packet_bin) = self.try_receive_bin().await? {
            let packet: mountain_mqtt::packets::packet_generic::PacketGeneric<'_, P, 0, 0> =
                packet_bin.as_packet_generic()?;
            let event = self.client_state.receive(packet)?;
            match event {
                ClientStateReceiveEvent::Ack => {
                    // We should now start sending pings - start from when we started connection,
                    // since this is the last time we sent a packet
                    self.ping_interval_start = self.connection_start;
                    info!("Client connected");
                }
                _ => {
                    return Err(ClientError::ClientState(
                        ClientStateError::ReceivedPacketOtherThanConnackOrAuthWhenConnecting,
                    ))
                }
            }
            // }
            // Delay.delay_ms(1).await;
        }

        Ok(())
    }

    /// Send a ping - does not check whether one is needed, but will update the ping interval start
    /// Cancel-safe: will either queue a ping to be sent and update state and reset the interval
    /// appropriately, or do nothing
    async fn ping(&mut self) -> Result<(), ClientError> {
        info!("Pinging");
        // CANCEL-SAFETY: We need to send the ping first, then
        // update the client_state and interval as sync operations.
        // This does involve just ignoring the
        // `Pingreq` packet we get back from the client state.
        // Either this async fn is dropped at the await point, and since
        // `send` is client safe, no packet is sent, OR we are not dropped,
        // the packet is sent, and we update the client state and interval.
        // Note that the packet is not actually sent immediately, it is just queued,
        // but if the actual sending fails then the client will error and should not
        // be used further.
        self.raw_client.send(Pingreq::default()).await?;
        self.client_state.send_ping()?;
        self.ping_interval_start = Some(Instant::now());
        Ok(())
    }

    /// Send a ping if more than the ping interval has elapsed (see [`Settings`]),
    /// and reset the ping interval if one was sent.
    /// Returns true if a ping was sent.
    /// Cancel-safe: Will either send a ping and update client state and ping interval,
    /// or do nothing.
    pub async fn ping_if_needed(&mut self) -> Result<bool, ClientError> {
        if let Some(ping_interval_start) = self.ping_interval_start {
            if ping_interval_start.elapsed() > self.settings.ping_interval {
                self.ping().await?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn check_receive_timeout(&self) -> Result<(), ClientError> {
        if let Some(receive_timeout_start) = self.receive_timeout_start {
            if receive_timeout_start.elapsed() > self.settings.receive_timeout {
                return Err(ClientError::ReceiveTimeoutServerUnresponsive);
            }
        }
        Ok(())
    }

    fn reset_receive_timeout(&mut self) {
        self.receive_timeout_start = Some(Instant::now());
    }

    /// Check whether a new [`PacketBin`] is available immediately, and if so
    /// return it.
    /// Note that this method is still async - it will not await new data from the server,
    /// but if data is already present then we may need to perform async operations, for example
    /// sending a response. However these operations will either complete or result in an error
    /// in a bounded time (e.g. if data cannot be sent, the server will disconnect us for lack
    /// of response, and/or we will be unable to ping the server and so the client will disconnect
    /// since the server will appear unresponsive).
    /// This will handle sending pings as needed, and will also detect if the server is
    /// unresponsive, and will then return an error.
    /// NOT CANCEL-SAFE (e.g. it can send data to the server)
    pub async fn try_receive_bin(&mut self) -> Result<Option<PacketBin<N>>, ClientError> {
        self.ping_if_needed().await?;

        let packet = self.raw_client.try_receive_bin();
        if packet.is_some() {
            self.reset_receive_timeout();
            Ok(packet)
        } else {
            self.check_receive_timeout()?;
            Ok(packet)
        }
    }

    async fn wait_for_interval(start: Option<Instant>, interval: Duration) {
        if let Some(start) = start {
            Timer::at(start + interval).await
        } else {
            // When start is None, check is not enabled, so never complete
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
    pub async fn receive_bin(&mut self) -> Result<PacketBin<N>, ClientError> {
        // Loop until we error or return a packet
        loop {
            // We need to deal with the next event - this is either the ping interval
            // elapsing, a receive timeout, or a packet arriving.
            // Note that all of these operations are cancel-safe
            let r = select3(
                Self::wait_for_interval(self.ping_interval_start, self.settings.ping_interval),
                Self::wait_for_interval(self.receive_timeout_start, self.settings.receive_timeout),
                self.raw_client.receive_bin(),
            )
            .await;

            match r {
                // Note that ping is cancel-safe
                Either3::First(()) => self.ping().await?,
                Either3::Second(()) => return Err(ClientError::ReceiveTimeoutServerUnresponsive),
                Either3::Third(packet_bin) => {
                    self.reset_receive_timeout();
                    return Ok(packet_bin);
                }
            };
        }
    }

    /// Request a subscription.
    /// This may require a response from the server, so after calling this, you must receive messages until
    /// [`PollClient::waiting_for_responses`] returns false, before calling any other methods that may
    /// require a response from the server.
    /// NOT CANCEL-SAFE
    pub async fn subscribe<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError> {
        let packet = self.client_state.subscribe(topic_name, maximum_qos)?;
        self.raw_client.send(packet).await
    }

    /// True if client is waiting for a response from the server - if this is true, then you must receive and
    /// handle packets until it becomes false, before attempting to send any more packets.
    /// This is done by calling [`PollClient::receive_bin`] or [`PollClient::try_receive_bin`]
    /// and then passing any resulting packet to [`PollClient::handle_packet_bin`], and
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
        self.raw_client.send(packet).await
    }

    /// Handle a [`PacketBin`], parsing it as a [`PacketGeneric`], then updating client state,
    /// sending any required response packet, and finally returning any [`ClientReceivedEvent`]
    /// resulting from the packet.
    /// This be called exactly once with each received [`PacketBin`]
    /// NOT CANCEL-SAFE (for example it may send response packets to the server)
    pub async fn handle_packet_bin<'b>(
        &mut self,
        packet_bin: &'b PacketBin<N>,
    ) -> Result<ClientReceivedEvent<'b, P>, ClientError> {
        let (event, to_send) = {
            let packet: PacketGeneric<'_, P, 0, 0> = packet_bin.as_packet_generic()?;

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
            self.raw_client.send(packet).await?;
        }

        Ok(event)
    }
}
