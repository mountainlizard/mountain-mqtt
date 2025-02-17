// use std::net::Ipv4Addr;

// use core::net::SocketAddr;
// use embedded_io_adapters::tokio_1::FromTokio;
// use heapless::Vec;
// use mountain_mqtt::{
//     client::{self, ClientNoQueue},
//     data::{
//         packet_identifier::{PacketIdentifier, PublishPacketIdentifier},
//         quality_of_service::QualityOfService,
//         reason_code::{ConnectReasonCode, SubscribeReasonCode, UnsubscribeReasonCode},
//     },
//     packet_client::PacketClient,
//     packets::{
//         connect::Connect,
//         disconnect::Disconnect,
//         packet_generic::PacketGeneric,
//         publish::Publish,
//         suback::Suback,
//         subscribe::{Subscribe, SubscriptionRequest},
//         unsuback::Unsuback,
//         unsubscribe::Unsubscribe,
//     },
// };
// use tokio::net::TcpStream;

// #[tokio::test]
// async fn connect_subscribe_and_publish() {

//     let ip = Ipv4Addr::new(127, 0, 0, 1);
//     let port = 1883;

//     let addr = SocketAddr::new(ip.into(), port);
//     let connection = TcpStream::connect(addr).await.unwrap();
//     let connection = FromTokio::new(connection);

//     let mut buf = [0;1024];
//     let client = ClientNoQueue::new(connection, &mut buf, delay, timeout_millis, message_handler)

//     const CLIENT_ID: &str = "mountain-mqtt-test-client-connect_subscribe_and_publish";
//     const TOPIC_NAME: &str = "mountain-mqtt-test-topic-connect_subscribe_and_publish";
//     const PAYLOAD: &[u8] = "mountain-mqtt-test-payload-connect_subscribe_and_publish".as_bytes();

//     // Note we can reuse this packet identifier for subscribe and unsubscribe since
//     // we wait for the subscribe to be acked before we send unsibscribe, freeing the
//     // identifier
//     const PACKET_IDENTIFIER: PacketIdentifier = PacketIdentifier(1234);

//     let mut buf = [0; 1024];
//     let mut client = tokio_localhost_client_connected(CLIENT_ID, &mut buf).await;

//     let primary_request = SubscriptionRequest::new(TOPIC_NAME, QualityOfService::QoS0);
//     let subscribe: Subscribe<'_, 0, 0> =
//         Subscribe::new(PACKET_IDENTIFIER, primary_request, Vec::new(), Vec::new());
//     client.send(subscribe).await.unwrap();
//     {
//         let maybe_suback: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();
//         assert_eq!(
//             maybe_suback,
//             PacketGeneric::Suback(Suback::new(
//                 PACKET_IDENTIFIER,
//                 SubscribeReasonCode::Success,
//                 Vec::new(),
//                 Vec::new()
//             ))
//         );
//     }

//     let publish: Publish<'_, 0> = Publish::new(
//         false,
//         false,
//         TOPIC_NAME,
//         PublishPacketIdentifier::None,
//         PAYLOAD,
//         Vec::new(),
//     );
//     client.send(publish).await.unwrap();

//     {
//         let maybe_publish: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();

//         assert_eq!(
//             maybe_publish,
//             PacketGeneric::Publish(Publish::new(
//                 false,
//                 false,
//                 TOPIC_NAME,
//                 PublishPacketIdentifier::None,
//                 PAYLOAD,
//                 Vec::new()
//             ))
//         );
//     }

//     let unsubscribe: Unsubscribe<'_, 0, 0> =
//         Unsubscribe::new(PACKET_IDENTIFIER, TOPIC_NAME, Vec::new(), Vec::new());
//     client.send(unsubscribe).await.unwrap();
//     {
//         let maybe_unsuback: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();

//         assert_eq!(
//             maybe_unsuback,
//             PacketGeneric::Unsuback(Unsuback::new(
//                 PACKET_IDENTIFIER,
//                 UnsubscribeReasonCode::Success,
//                 Vec::new(),
//                 Vec::new()
//             ))
//         );
//     }

//     client.send(Disconnect::default()).await.unwrap();
// }
