use heapless::Vec;
use mountain_mqtt::{
    client::{Client, ClientNoQueue, Delay},
    data::quality_of_service::QualityOfService,
    packets::connect::Connect,
};
use std::time::Duration;
use tokio::{net::TcpStream, sync::mpsc};

#[derive(Clone)]
pub struct TokioDelay;

impl Delay for TokioDelay {
    async fn delay_us(&mut self, us: u32) {
        tokio::time::sleep(Duration::from_micros(us as u64)).await;
    }
}

/// Expects tp connect to an MQTT server on 127.0.0.1:1883,
/// server must accept connections with no username or password
#[tokio::test]
async fn client_connect_subscribe_and_publish() {
    let ip = core::net::Ipv4Addr::new(127, 0, 0, 1);
    let port = 1883;

    let addr = core::net::SocketAddr::new(ip.into(), port);
    let connection = TcpStream::connect(addr).await.unwrap();

    let delay = TokioDelay;

    let (message_tx, mut message_rx) = mpsc::channel(32);

    let mut buf = [0; 1024];
    let mut client =
        ClientNoQueue::new(connection, &mut buf, delay, 5000, |topic_name, payload| {
            message_tx
                .try_send((topic_name.to_owned(), payload.to_vec()))
                .unwrap();
            Ok(())
        });

    const CLIENT_ID: &str = "mountain-mqtt-test-client-client_connect_subscribe_and_publish";
    const TOPIC_NAME: &str = "mountain-mqtt-test-topic-client_connect_subscribe_and_publish";
    const PAYLOAD: &[u8] =
        "mountain-mqtt-test-payload-client_connect_subscribe_and_publish".as_bytes();
    const PAYLOAD2: &[u8] =
        "mountain-mqtt-test-payload2-client_connect_subscribe_and_publish".as_bytes();

    let connect: Connect<'_, 0> = Connect::new(60, None, None, CLIENT_ID, true, None, Vec::new());
    client.connect(connect).await.unwrap();

    client
        .subscribe_to_topic(TOPIC_NAME, &QualityOfService::QoS0)
        .await
        .unwrap();

    client
        .send_message(TOPIC_NAME, PAYLOAD, QualityOfService::QoS0, false)
        .await
        .unwrap();

    // Normally we would poll continuously with wait = false until we get an error/disconnect,
    // in this case we know we are expecting just the publish packet from the server
    let received = client.poll(true).await.unwrap();
    assert!(received);

    // Send another message
    client
        .send_message(TOPIC_NAME, PAYLOAD2, QualityOfService::QoS0, false)
        .await
        .unwrap();
    let received = client.poll(true).await.unwrap();
    assert!(received);

    // Check we got the messages through
    assert_eq!(
        message_rx.try_recv(),
        Ok((TOPIC_NAME.to_owned(), PAYLOAD.to_vec()))
    );

    assert_eq!(
        message_rx.try_recv(),
        Ok((TOPIC_NAME.to_owned(), PAYLOAD2.to_vec()))
    );

    client.unsubscribe_from_topic(TOPIC_NAME).await.unwrap();

    client.disconnect().await.unwrap();
}
