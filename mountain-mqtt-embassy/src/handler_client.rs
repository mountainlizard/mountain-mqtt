use embassy_sync::blocking_mutex::raw::RawMutex;
use heapless::Vec;
use mountain_mqtt::{
    client::{ClientError, ClientReceivedEvent, EventHandlerError},
    client_state::ClientState,
    data::{property::PublishProperty, quality_of_service::QualityOfService},
};

use crate::{packet_bin::PacketBin, poll_client::PollClient};

/// An MQTT client that extends a [`PollClient`] with a handler function
/// for accepting received messages, allowing for more convenient use.
/// Note that the handler is not async to avoid cancel-safety issues
/// and also to ensure that the client keeps running. If you wish to
/// perform async operations in response to events, consider using a channel
/// to queue them, with [`embassy_sync::channel::Channel::try_send`] to
/// send the events onwards.
pub struct HandlerClient<'a, S, M, F, const N: usize, const P: usize>
where
    M: RawMutex,
    S: ClientState,
    F: Fn(ClientReceivedEvent<P>) -> Result<(), EventHandlerError>,
{
    poll_client: PollClient<'a, S, M, N, P>,
    handler: F,
    pending_packet: Option<PacketBin<N>>,
}

impl<'a, S, M, F, const N: usize, const P: usize> HandlerClient<'a, S, M, F, N, P>
where
    M: RawMutex,
    S: ClientState,
    F: Fn(ClientReceivedEvent<P>) -> Result<(), EventHandlerError>,
{
    pub fn new(poll_client: PollClient<'a, S, M, N, P>, handler: F) -> Self {
        Self {
            poll_client,
            handler,
            pending_packet: None,
        }
    }

    /// Consume this [`HandlerClient`] and return the underlying [`PollClient`]
    pub fn to_poll_client(self) -> PollClient<'a, S, M, N, P> {
        self.poll_client
    }

    /// NOT CANCEL-SAFE: When packets are received, they must then be processed, so this future should
    /// not be dropped unless this client or the associater [`PollClient`]` will not be used again.
    /// TODO: If self.receive becomes cancel-safe, this will be too
    async fn wait_for_response(&mut self) -> Result<(), ClientError> {
        while self.poll_client.waiting_for_responses() {
            self.receive().await?;
        }

        Ok(())
    }

    /// Attempt to receive and handle one message
    /// Cancel-safe: If this is interrupted, it will cache any pending packet and process it
    /// the next time this method is called.
    pub async fn receive(&mut self) -> Result<(), ClientError> {
        // If we have a pending packet, use it, otherwise receive a packet and make that pending
        let packet = if let Some(packet) = self.pending_packet {
            packet
        } else {
            // Cancel-safety: If we're interrupted here we won't store a packet as pending
            let packet = self.poll_client.receive().await?;
            self.pending_packet = Some(packet);
            packet
        };

        // Cancel-safety: If we're interrupted here we'll leave the packet in pending_packet,
        // and try again to process it next time we're called
        let event = self.poll_client.process(&packet).await?;

        // Cancel-safety: If we make it here the packet is processed, so we can clear pending
        // Since we've already updated client state we can't allow an interruption, so handler
        // cannot be async. In theory we could possibly allow an async handler by having another
        // stage of processing, where we first produce an event to pass to the handler, and if
        // this completes, only then do we call poll_client.process, which updates state and
        // possibly sends a packet. However then we would have to require that the handler is
        // cancel safe, and it would receive the same event again if the handler was cancelled.
        self.pending_packet = None;
        let handler = &self.handler;
        handler(event).map_err(ClientError::EventHandler)?;

        Ok(())
    }

    /// Publish a message with given payload to a given topic, with no properties.
    /// This will then wait for any required response from the server.
    /// NOT CANCEL-SAFE: This method will wait for responses, and when packets are received, they must
    /// then be processed, so this future should not be dropped unless this client or the associater
    /// [`PollClient`]` will not be used again.
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
    /// This will then wait for any required response from the server.
    /// NOT CANCEL-SAFE: This method will wait for responses, and when packets are received, they must
    /// then be processed, so this future should not be dropped unless this client or the associater
    /// [`PollClient`]` will not be used again.
    pub async fn publish_with_properties<'b, const PP: usize>(
        &'b mut self,
        topic_name: &'b str,
        payload: &'b [u8],
        qos: QualityOfService,
        retain: bool,
        properties: Vec<PublishProperty<'b>, PP>,
    ) -> Result<(), ClientError> {
        self.poll_client
            .publish_with_properties(topic_name, payload, qos, retain, properties)
            .await?;
        self.wait_for_response().await?;
        Ok(())
    }

    /// Request a subscription.
    /// This will then wait for any required response from the server.
    /// NOT CANCEL-SAFE: This method will wait for responses, and when packets are received, they must
    /// then be processed, so this future should not be dropped unless this client or the associater
    /// [`PollClient`]` will not be used again.
    pub async fn subscribe(
        &mut self,
        topic_name: &str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError> {
        self.poll_client.subscribe(topic_name, maximum_qos).await?;
        self.wait_for_response().await?;
        Ok(())
    }

    /// Request to unsubscribe from a topic
    /// This will then wait for any required response from the server.
    /// NOT CANCEL-SAFE: This method will wait for responses, and when packets are received, they must
    /// then be processed, so this future should not be dropped unless this client or the associater
    /// [`PollClient`]` will not be used again.
    pub async fn unsubscribe(&mut self, topic_name: &str) -> Result<(), ClientError> {
        self.poll_client.unsubscribe(topic_name).await?;
        self.wait_for_response().await?;
        Ok(())
    }
}
