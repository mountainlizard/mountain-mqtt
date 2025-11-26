use defmt::info;
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::{Receiver, Sender, TryReceiveError},
};
use heapless::Vec;
use mountain_mqtt::{
    client::{ClientError, ClientReceivedEvent, ConnectionSettings},
    client_state::{ClientState, ClientStateError, ClientStateNoQueue, ClientStateReceiveEvent},
    data::{property::ConnectProperty, quality_of_service::QualityOfService},
    packets::{
        connect::{Connect, Will},
        packet_generic::PacketGeneric,
    },
};

use crate::{packet_bin::PacketBin, raw_client::RawClient};

// /// The result of calling [`PollClient::receive`] or [`PollClient::try_receive`]
// pub struct Received<'a, const N: usize, const P: usize> {
//     packet_bin: PacketBin<N>,
//     event: Option<ClientReceivedEvent<'a, P>>,
// }

// impl <'a, const N: usize, const P: usize> Received<'a, N, P> {
//     pub async fn new(
//         packet_bin: PacketBin<N>,
//     ) -> Result<, ClientError> {
// }

// impl<'a, const N: usize, const P: usize> Received<'a, N, P> {
//     pub fn packet_bin(&self) -> &PacketBin<N> {
//         &self.packet_bin
//     }
//     pub fn event(&self) -> &Option<ClientReceivedEvent<'a, P>> {
//         &self.event
//     }
// }

// /// The result of calling [`PollClient::receive`] or [`PollClient::try_receive`]
// pub struct Received<'a, const N: usize, const P: usize> {
//     packet_bin: PacketBin<N>,
//     packet: Option<PacketGeneric<'a, P, 0, 0>>,
// }

// impl<'a, const N: usize, const P: usize> Received<'a, N, P> {
//     pub async fn new(packet_bin: PacketBin<N>) -> Result<Self, ClientError> {
//         let mut r = Self {
//             packet_bin,
//             packet: None,
//         };
//         r.packet = r.packet_bin.as_packet_generic().ok();
//         // let packet = packet_bin.as_packet_generic()?;
//         // Ok(Self { packet_bin, packet })
//         Ok(r)
//     }
// }

// impl<'a, const N: usize, const P: usize> Received<'a, N, P> {
//     pub fn packet_bin(&self) -> &PacketBin<N> {
//         &self.packet_bin
//     }
//     pub fn packet(&self) -> &Option<PacketGeneric<'a, P, 0, 0>> {
//         &self.packet
//     }
// }

pub struct PollClient<'a, M, const N: usize, const P: usize>
where
    M: RawMutex,
{
    client_state: ClientStateNoQueue,
    raw_client: RawClient<'a, M, N>,
}

impl<'a, M, const N: usize, const P: usize> PollClient<'a, M, N, P>
where
    M: RawMutex,
{
    pub fn new(
        sender: Sender<'a, M, PacketBin<N>, 1>,
        receiver: Receiver<'a, M, PacketBin<N>, 1>,
    ) -> Self {
        Self {
            client_state: ClientStateNoQueue::default(),
            raw_client: RawClient::new(sender, receiver),
        }
    }

    pub async fn connect(&mut self, settings: &ConnectionSettings<'_>) -> Result<(), ClientError> {
        self.connect_with_will::<0>(settings, None).await
    }

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
        self.client_state.connect(&packet)?;
        self.raw_client.send(packet).await?;

        // All that can happen after first connecting is that we receive an Ack,
        // indicating we are connected and can continue, or
        self.wait_for_ack_only().await?;

        Ok(())
    }

    async fn wait_for_ack_only(&mut self) -> Result<(), ClientError> {
        while self.client_state.waiting_for_responses() {
            let packet_bin = self.raw_client.receive_bin().await;
            let packet: mountain_mqtt::packets::packet_generic::PacketGeneric<'_, P, 0, 0> =
                packet_bin.as_packet_generic()?;
            let event = self.client_state.receive(packet)?;
            match event {
                ClientStateReceiveEvent::Ack => {
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

    pub async fn receive_bin(&mut self) -> PacketBin<N> {
        self.raw_client.receive_bin().await
    }

    pub async fn try_receive_bin(&mut self) -> Option<PacketBin<N>> {
        self.raw_client.try_receive_bin().await
    }

    // pub async fn try_receive(&mut self) -> Result<Option<Received<N, P>>, ClientError> {
    //     let packet_bin = self.raw_client.try_receive_bin().await;
    //     match packet_bin {
    //         Ok(packet_bin) => {
    //             let event = self.handle_packet_bin(&packet_bin).await?;
    //             Ok(Some(Received { packet_bin, event }))
    //         }
    //         _ => Ok(None),
    //     }
    // }

    pub async fn subscribe<'b>(
        &'b mut self,
        topic_name: &'b str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError> {
        let packet = self.client_state.subscribe(topic_name, maximum_qos)?;
        self.raw_client.send(packet).await
    }

    /// True if client is waiting for responses - if this is true, then you must receive and
    /// handle packets until it becomes false, before attempting to send any more packets.
    /// This is done by calling [`PollClient::receive_bin`] or [`PollClient::try_receive_bin`]
    /// and then passing any resulting packet to [`PollClient::handle_packet_bin`], and
    /// handling any resulting [`ClientReceivedEvent`]s.
    pub fn waiting_for_responses(&self) -> bool {
        self.client_state.waiting_for_responses()
    }

    /// Handle a [`PacketBin`], parsing it as a [`PacketGeneric`], then updating client state,
    /// sending any required response packet, and finally returning any [`ClientReceivedEvent`]
    /// resulting from the packet.
    /// This be called exactly once with each received [`PacketBin`]
    pub async fn handle_packet_bin<'b>(
        &mut self,
        packet_bin: &'b PacketBin<N>,
    ) -> Result<Option<ClientReceivedEvent<'b, P>>, ClientError> {
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

        Ok(Some(event))
    }
}
