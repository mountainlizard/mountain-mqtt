use std::net::Ipv4Addr;

use core::net::SocketAddr;
use embedded_io_adapters::tokio_1::FromTokio;
use heapless::Vec;
use mountain_mqtt::{
    data::{
        packet_identifier::{PacketIdentifier, PublishPacketIdentifier},
        quality_of_service::QualityOfService,
        reason_code::SubscriptionReasonCode,
    },
    packet_client::PacketClient,
    packets::{
        connect::Connect,
        disconnect::Disconnect,
        packet_generic::PacketGeneric,
        publish::Publish,
        subscribe::{Subscribe, SubscriptionRequest},
        unsubscribe::Unsubscribe,
    },
};
use tokio::net::TcpStream;

/// Create a client ready to connect to MQTT server on 127.0.0.1:1883,
/// server must accept connections with no username or password
async fn tokio_localhost_client(buf: &mut [u8]) -> PacketClient<'_, FromTokio<TcpStream>> {
    let ip = Ipv4Addr::new(127, 0, 0, 1);
    let port = 1883;

    let addr = SocketAddr::new(ip.into(), port);
    let connection = TcpStream::connect(addr).await.unwrap();
    let connection = FromTokio::new(connection);

    PacketClient::new(connection, buf)
}

/// Note - be sure to use a different client id for each test, and generally a different
/// topic as well, to prevent issues caused by tests running in parallel. E.g. if two tests
/// share a client id, and B starts a connection while A is already connected, A will be
/// disconnected by the server with a SessionTakenOver reason code. Similarly, sharing topics
/// may cause tests to fail from tests seeing publish messages from the server caused by
/// other tests publishing to a topic.
async fn tokio_localhost_client_connected<'a>(
    client_id: &'a str,
    buf: &'a mut [u8],
) -> PacketClient<'a, FromTokio<TcpStream>> {
    let mut client = tokio_localhost_client(buf).await;

    let connect: Connect<'_, 0> = Connect::new(120, None, None, client_id, true, None, Vec::new());
    client.send(connect).await.unwrap();

    {
        let maybe_connack: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();
        // TODO: Change to assert_eq! to an expected packet
        assert!(matches!(maybe_connack, PacketGeneric::Connack(_)));
    }

    client
}

#[tokio::test]
async fn connect_and_publish() {
    const CLIENT_ID: &str = "mountain-mqtt-test-client-connect_and_publish";
    const TOPIC_NAME: &str = "mountain-mqtt-test-topic-connect_and_publish";
    const PAYLOAD: &[u8] = "mountain-mqtt-test-payload-connect_and_publish".as_bytes();

    let mut buf = [0; 1024];
    let mut client = tokio_localhost_client_connected(CLIENT_ID, &mut buf).await;

    let publish: Publish<'_, 0> = Publish::new(
        false,
        false,
        TOPIC_NAME,
        PublishPacketIdentifier::None,
        PAYLOAD,
        Vec::new(),
    );
    client.send(publish).await.unwrap();

    client.send(Disconnect::default()).await.unwrap();
}

#[tokio::test]
async fn connect_subscribe_and_publish() {
    const CLIENT_ID: &str = "mountain-mqtt-test-client-connect_subscribe_and_publish";
    const TOPIC_NAME: &str = "mountain-mqtt-test-topic-connect_subscribe_and_publish";
    const PAYLOAD: &[u8] = "mountain-mqtt-test-payload-connect_subscribe_and_publish".as_bytes();

    // Note we can reuse this packet identifier for subscribe and unsubscribe since
    // we wait for the subscribe to be acked before we send unsibscribe, freeing the
    // identifier
    const PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(1234);

    let mut buf = [0; 1024];
    let mut client = tokio_localhost_client_connected(CLIENT_ID, &mut buf).await;

    let primary_request = SubscriptionRequest::new(TOPIC_NAME, QualityOfService::QoS0);
    let subscribe: Subscribe<'_, 0, 0> =
        Subscribe::new(PACKET_IDENTIFIER, primary_request, Vec::new(), Vec::new());
    client.send(subscribe).await.unwrap();
    {
        let maybe_suback: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();
        // TODO: change to comparing an expected packet to reduce to a single assert_eq!
        match maybe_suback {
            PacketGeneric::Suback(suback) => {
                assert_eq!(suback.packet_identifier(), &PACKET_IDENTIFIER);
                assert_eq!(suback.reason_codes().len(), 1);
                assert_eq!(
                    suback.reason_codes().first(),
                    Some(&SubscriptionReasonCode::Success)
                );
                assert!(suback.properties().is_empty());
            }
            _ => panic!("Expected Suback, got {:?}", maybe_suback),
        }
    }

    let publish: Publish<'_, 0> = Publish::new(
        false,
        false,
        TOPIC_NAME,
        PublishPacketIdentifier::None,
        PAYLOAD,
        Vec::new(),
    );
    client.send(publish).await.unwrap();

    {
        let maybe_publish: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();
        // TODO: change to comparing an expected packet to reduce to a single assert_eq!
        match maybe_publish {
            PacketGeneric::Publish(publish) => {
                assert!(!publish.duplicate());
                assert!(!publish.retain());
                assert_eq!(publish.topic_name(), TOPIC_NAME);
                assert_eq!(
                    publish.publish_packet_identifier(),
                    &PublishPacketIdentifier::None
                );
                assert_eq!(publish.payload(), PAYLOAD);
                assert!(publish.properties().is_empty());
            }
            _ => panic!("Expected Publish, got {:?}", maybe_publish),
        }
    }

    let unsubscribe: Unsubscribe<'_, 0, 0> =
        Unsubscribe::new(PACKET_IDENTIFIER, TOPIC_NAME, Vec::new(), Vec::new());
    client.send(unsubscribe).await.unwrap();
    {
        let maybe_unsuback: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();
        // TODO: assert_eq! to expected example packet
        assert!(matches!(maybe_unsuback, PacketGeneric::Unsuback(_)));
    }

    client.send(Disconnect::default()).await.unwrap();
}
