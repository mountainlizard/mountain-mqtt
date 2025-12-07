use embassy_sync::blocking_mutex::raw::RawMutex;
use heapless::Vec;
use mountain_mqtt::{
    client::{Client, ClientError, ClientReceivedEvent, ConnectionSettings, EventHandlerError},
    client_state::ClientState,
    data::{property::PublishProperty, quality_of_service::QualityOfService},
    packets::connect::Will,
};

use crate::{packet_bin::PacketBin, poll_client::PollClient};

pub trait SyncEventHandler<const P: usize> {
    fn handle_event(&mut self, event: ClientReceivedEvent<P>) -> Result<(), EventHandlerError>;
}

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
    F: SyncEventHandler<P>,
{
    poll_client: PollClient<'a, S, M, N, P>,
    handler: F,
    // The pending packet, if any. This is a packet that has been received,
    // but not yet processed.
    pending_packet: Option<PacketBin<N>>,
}

impl<'a, S, M, F, const N: usize, const P: usize> HandlerClient<'a, S, M, F, N, P>
where
    M: RawMutex,
    S: ClientState,
    F: SyncEventHandler<P>,
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

    /// Receive packets until all pending responses are received (e.g. acknowledgement of subscribe,
    /// unsubscribe, publish)
    /// Cancel-safe: Note that if this is cancelled, it shold be called again until successful,
    /// to be sure that responses have been received.
    async fn wait_for_responses(&mut self) -> Result<(), ClientError> {
        while self.poll_client.waiting_for_responses() {
            self.receive().await?;
        }

        Ok(())
    }

    /// Check whether a new packet is available immediately, and if so handle
    /// it.
    /// Note that this method is still async - it will not await new data from the server,
    /// but it may need to perform other async operations, e.g. sending a ping. However these
    /// operations will either complete or result in an error in a bounded time (e.g. if data
    /// cannot be sent, the server will disconnect us for lack of response, and/or we will
    /// be unable to ping the server and so the client will disconnect since the server will
    /// appear unresponsive).
    /// Returns true if a packet was received and handled.
    /// This will handle sending pings as needed, and check if the server is unresponsive.
    /// Cancel-safe
    pub async fn try_receive(&mut self) -> Result<bool, ClientError> {
        // See if we can get a pending packet
        // If receive is interrupted or fails, pending_packet is left as None so we will try
        // to receive next time
        if self.pending_packet.is_none() {
            self.pending_packet = self.poll_client.try_receive().await?;
        }

        // Only run receive logic if we've actually got a packet
        if let Some(ref packet) = self.pending_packet {
            let event = self.poll_client.process(packet).await?;

            // Cancel-safety: If we make it here the packet is processed, so we need to make sure
            // we always clear the pending packet. This means the handler cannot be async since it
            // could be interrupted.
            // In theory we could possibly allow an async handler by having another stage of
            // processing, where we first produce an event to pass to the handler, and if
            // this completes, only then do we call poll_client.process, which updates state and
            // possibly sends a packet. However then we would have to require that the handler is
            // cancel-safe, and it would receive the same event again if the handler was cancelled.

            // Note we don't return any error immediately with "?" - we need to make sure we clear
            // the pending packet before exiting this method. We can't clear the pending packet
            // before calling handler since the event lifetime is linked to it.
            let handler_result = self
                .handler
                .handle_event(event)
                .map_err(ClientError::EventHandler);

            // Pending packet has been processed and passed to handler, we can clear it
            self.pending_packet = None;

            // We're now safe to exit method
            handler_result?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Attempt to receive and handle one message
    /// Cancel-safe: If this is interrupted, it will cache any pending packet and process it
    /// the next time this method is called.
    pub async fn receive(&mut self) -> Result<(), ClientError> {
        // Make sure we have a pending packet (inserting a received packet if we have none),
        // and get a ref to it
        // If receive is interrupted or fails, pending_packet is left as None so we will try
        // to receive next time
        let packet = self
            .pending_packet
            .get_or_insert(self.poll_client.receive().await?);

        // Cancel-safety: If we're interrupted here we'll leave the packet in pending_packet,
        // and try again to process it next time we're called
        let event = self.poll_client.process(packet).await?;

        // Cancel-safety: If we make it here the packet is processed, so we need to make sure
        // we always clear the pending packet. This means the handler cannot be async since it
        // could be interrupted.
        // In theory we could possibly allow an async handler by having another stage of
        // processing, where we first produce an event to pass to the handler, and if
        // this completes, only then do we call poll_client.process, which updates state and
        // possibly sends a packet. However then we would have to require that the handler is
        // cancel-safe, and it would receive the same event again if the handler was cancelled.

        // Note we don't return any error immediately with "?" - we need to make sure we clear
        // the pending packet before exiting this method. We can't clear the pending packet
        // before calling handler since the event lifetime is linked to it.
        let handler_result = self
            .handler
            .handle_event(event)
            .map_err(ClientError::EventHandler);

        // Pending packet has been processed and passed to handler, we can clear it
        self.pending_packet = None;

        // We're now safe to exit method
        handler_result?;
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
}

impl<'a, S, M, F, const N: usize, const P: usize> Client<'a> for HandlerClient<'a, S, M, F, N, P>
where
    M: RawMutex,
    S: ClientState,
    F: SyncEventHandler<P>,
{
    async fn connect(&mut self, settings: &ConnectionSettings<'_>) -> Result<(), ClientError> {
        self.poll_client.connect(settings).await
    }

    async fn connect_with_will<const W: usize>(
        &mut self,
        settings: &ConnectionSettings<'_>,
        will: Option<Will<'_, W>>,
    ) -> Result<(), ClientError> {
        self.poll_client.connect_with_will(settings, will).await?;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), ClientError> {
        self.poll_client.disconnect().await
    }

    async fn send_ping(&mut self) -> Result<(), ClientError> {
        // This is a no-op since we already handle sending pings as required
        Ok(())
    }

    async fn poll(&mut self, wait: bool) -> Result<bool, ClientError> {
        if wait {
            self.receive().await?;
            Ok(true)
        } else {
            self.try_receive().await
        }
    }

    /// Request a subscription.
    /// This will then wait for any required response from the server.
    /// NOT CANCEL-SAFE: If cancelled, there may be pending responses from the server, and this may result
    /// in an error if another client method is called that attempts to send requests (e.g. subscribe, unsubscribe,
    /// publish). To avoid this, if this method is cancelled, ensure you call [`HandlerClient::wait_for_responses`]
    /// and allow it to complete before attempting to send further requests.
    async fn subscribe(
        &mut self,
        topic_name: &str,
        maximum_qos: QualityOfService,
    ) -> Result<(), ClientError> {
        self.poll_client.subscribe(topic_name, maximum_qos).await?;
        self.wait_for_responses().await?;
        Ok(())
    }

    /// Request to unsubscribe from a topic
    /// This will then wait for any required response from the server.
    /// NOT CANCEL-SAFE: If cancelled, there may be pending responses from the server, and this may result
    /// in an error if another client method is called that attempts to send requests (e.g. subscribe, unsubscribe,
    /// publish). To avoid this, if this method is cancelled, ensure you call [`HandlerClient::wait_for_responses`]
    /// and allow it to complete before attempting to send further requests.
    async fn unsubscribe(&mut self, topic_name: &str) -> Result<(), ClientError> {
        self.poll_client.unsubscribe(topic_name).await?;
        self.wait_for_responses().await?;
        Ok(())
    }

    /// Publish a message with given payload to a given topic, with given properties
    /// This will then wait for any required response from the server.
    /// NOT CANCEL-SAFE: If cancelled, there may be pending responses from the server, and this may result
    /// in an error if another client method is called that attempts to send requests (e.g. subscribe, unsubscribe,
    /// publish). To avoid this, if this method is cancelled, ensure you call [`HandlerClient::wait_for_responses`]
    /// and allow it to complete before attempting to send further requests.
    async fn publish_with_properties<'b, const PP: usize>(
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
        self.wait_for_responses().await?;
        Ok(())
    }
}
