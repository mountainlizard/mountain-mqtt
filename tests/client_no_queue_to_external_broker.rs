use mountain_mqtt::{
    client::{Client, ClientNoQueue, ClientReceivedEvent, ConnectionSettings},
    data::quality_of_service::QualityOfService,
    tokio::{ConnectionTcpStream, TokioDelay},
};
use tokio::{net::TcpStream, sync::mpsc};

/// Expects tp connect to an MQTT server on 127.0.0.1:1883,
/// server must accept connections with no username or password
#[tokio::test]
async fn client_connect_subscribe_and_publish() {
    let ip = core::net::Ipv4Addr::new(127, 0, 0, 1);
    let port = 1883;

    let addr = core::net::SocketAddr::new(ip.into(), port);
    let tcp_stream = TcpStream::connect(addr).await.unwrap();
    let connection = ConnectionTcpStream::new(tcp_stream);

    let delay = TokioDelay;

    let (message_tx, mut message_rx) = mpsc::channel(32);

    let mut buf = [0; 1024];
    let mut client = ClientNoQueue::new(
        connection,
        &mut buf,
        delay,
        5000,
        |event: ClientReceivedEvent<'_, 16>| {
            if let ClientReceivedEvent::ApplicationMessage(message) = event {
                message_tx
                    .try_send((message.topic_name.to_owned(), message.payload.to_vec()))
                    .unwrap();
            }
            Ok(())
        },
    );

    const CLIENT_ID: &str = "mountain-mqtt-test-client-client_connect_subscribe_and_publish";
    const TOPIC_NAME: &str = "mountain-mqtt-test-topic-client_connect_subscribe_and_publish";
    const PAYLOAD: &[u8] =
        "mountain-mqtt-test-payload-client_connect_subscribe_and_publish".as_bytes();
    const PAYLOAD2: &[u8] =
        "mountain-mqtt-test-payload2-client_connect_subscribe_and_publish".as_bytes();

    client
        .connect(ConnectionSettings::unauthenticated(CLIENT_ID))
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
