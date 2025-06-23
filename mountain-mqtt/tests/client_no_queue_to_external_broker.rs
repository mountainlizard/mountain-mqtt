use mountain_mqtt::{
    client::{Client, ClientReceivedEvent, ConnectionSettings, EventHandler, EventHandlerError},
    data::quality_of_service::QualityOfService,
    tokio::client_tcp,
};
use tokio::sync::mpsc::{self, Sender};

/// Sends application message events onwards to a [Sender]
pub struct SenderEventHandler {
    sender: Sender<(String, Vec<u8>)>,
}

impl<const P: usize> EventHandler<P> for SenderEventHandler {
    async fn handle_event(
        &mut self,
        event: ClientReceivedEvent<'_, P>,
    ) -> Result<(), EventHandlerError> {
        if let ClientReceivedEvent::ApplicationMessage(message) = event {
            self.sender
                .send((message.topic_name.to_owned(), message.payload.to_vec()))
                .await
                .map_err(|_| EventHandlerError::Overflow)?;
        }
        Ok(())
    }
}

/// Expects tp connect to an MQTT server on 127.0.0.1:1883,
/// server must accept connections with no username or password
#[tokio::test]
async fn client_connect_subscribe_and_publish() {
    let ip = core::net::Ipv4Addr::new(127, 0, 0, 1);
    let port = 1883;
    let timeout_millis = 5000;
    let mut buf = [0; 1024];

    let (message_tx, mut message_rx) = mpsc::channel(32);

    let handler = SenderEventHandler { sender: message_tx };

    let mut client =
        client_tcp::<SenderEventHandler, 16>(ip, port, timeout_millis, &mut buf, handler).await;

    const CLIENT_ID: &str = "mountain-mqtt-test-client-client_connect_subscribe_and_publish";
    const TOPIC_NAME: &str = "mountain-mqtt-test-topic-client_connect_subscribe_and_publish";
    const PAYLOAD: &[u8] =
        "mountain-mqtt-test-payload-client_connect_subscribe_and_publish".as_bytes();
    const PAYLOAD2: &[u8] =
        "mountain-mqtt-test-payload2-client_connect_subscribe_and_publish".as_bytes();

    client
        .connect(&ConnectionSettings::unauthenticated(CLIENT_ID))
        .await
        .unwrap();

    client
        .subscribe(TOPIC_NAME, QualityOfService::Qos0)
        .await
        .unwrap();

    client
        .publish(TOPIC_NAME, PAYLOAD, QualityOfService::Qos0, false)
        .await
        .unwrap();

    // Normally we would poll continuously with wait = false until we get an error/disconnect,
    // in this case we know we are expecting just the publish packet from the server
    let received = client.poll(true).await.unwrap();
    assert!(received);

    // Send another message at qos1
    client
        .publish(TOPIC_NAME, PAYLOAD2, QualityOfService::Qos1, false)
        .await
        .unwrap();

    // We may have already received the message publish while waiting for QoS1
    // ack to be received, if not, poll for it
    if message_rx.len() < 2 {
        let received = client.poll(true).await.unwrap();
        assert!(received);
    }

    // Check we got the messages through
    assert_eq!(
        message_rx.try_recv(),
        Ok((TOPIC_NAME.to_owned(), PAYLOAD.to_vec()))
    );

    assert_eq!(
        message_rx.try_recv(),
        Ok((TOPIC_NAME.to_owned(), PAYLOAD2.to_vec()))
    );

    client.unsubscribe(TOPIC_NAME).await.unwrap();

    client.disconnect().await.unwrap();
}
