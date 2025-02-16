use std::net::Ipv4Addr;

use core::net::SocketAddr;
use embedded_io_adapters::tokio_1::FromTokio;
use heapless::Vec;
use mountain_mqtt::{
    data::packet_identifier::PublishPacketIdentifier,
    packet_client::PacketClient,
    packets::{
        connect::Connect, disconnect::Disconnect, packet_generic::PacketGeneric, publish::Publish,
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

async fn tokio_localhost_client_connected(
    buf: &mut [u8],
) -> PacketClient<'_, FromTokio<TcpStream>> {
    let mut client = tokio_localhost_client(buf).await;

    let connect: Connect<'_, 0> = Connect::new(
        120,
        None,
        None,
        "mountain-mqtt-test-client",
        true,
        None,
        Vec::new(),
    );
    client.send(connect).await.unwrap();

    {
        let maybe_connack: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();
        assert!(matches!(maybe_connack, PacketGeneric::Connack(_)));
    }

    client
}

#[tokio::test]
async fn connect_and_publish() {
    let mut buf = [0; 1024];
    let mut client = tokio_localhost_client_connected(&mut buf).await;

    let publish: Publish<'_, 0> = Publish::new(
        false,
        false,
        "mountain-mqtt-test-topic",
        PublishPacketIdentifier::None,
        "Hello from Mountain MQTT!".as_bytes(),
        Vec::new(),
    );
    client.send(publish).await.unwrap();

    client.send(Disconnect::default()).await.unwrap();
}
