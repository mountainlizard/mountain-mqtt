// use core::net::Ipv4Addr;
// use defmt::info;
// use embassy_futures::{
//     join::join3,
//     select::{select, Either},
// };
// use embassy_net::{
//     tcp::{ConnectError, TcpSocket},
//     Stack,
// };
// use embassy_sync::{
//     channel::{self, Channel},
//     pubsub::{PubSubChannel, Publisher, Subscriber},
// };
// use embassy_time::{with_timeout, Duration};
// use embedded_io_async::{Read, Write};
// use mountain_mqtt::packets::{packet::Packet, packet_generic::PacketGeneric};

// #[derive(Debug, Clone, Copy)]
// pub enum ChannelCloseReason {
//     Closed,
//     ReadError,
//     WriteError,
//     SocketError,
// }

// pub struct Settings {
//     /// The address of the MQTT server
//     pub address: Ipv4Addr,

//     /// The port of the MQTT server
//     pub port: u16,
// }

// pub enum State {
//     Idle,
//     Running,
//     Stopped,
// }

// #[derive(PartialEq, Eq, Clone, Copy, Debug)]
// #[cfg_attr(feature = "defmt", derive(defmt::Format))]
// pub enum Error {
//     ConnectError(ConnectError),
// }

// // const P: usize = 16;
// // const W: usize = 16;
// // const S: usize = 16;

// pub struct MessageChannel<'a, M, const P: usize, const W: usize, const S: usize>
// where
//     M: embassy_sync::blocking_mutex::raw::RawMutex,
// {
//     rx_channel_receiver: channel::Receiver<'a, M, PacketGeneric<'a, P, W, S>, 1>,
//     tx_channel_sender: channel::Sender<'a, M, PacketGeneric<'a, P, W, S>, 1>,
//     cancel_pub: Publisher<'a, M, ChannelCloseReason, 1, 3, 3>,
//     cancel_sub: Subscriber<'a, M, ChannelCloseReason, 1, 3, 3>,
// }

// // impl<'a, M, const P: usize, const W: usize, const S: usize> Drop for MessageChannel<'a, M, P, W, S>
// // where
// //     M: embassy_sync::blocking_mutex::raw::RawMutex,
// // {
// //     fn drop(&mut self) {
// //         drop(self.rx_channel_receiver);
// //         drop(self.tx_channel_sender);
// //         drop(self.cancel_pub);
// //         drop(self.cancel_sub);
// //     }
// // }

// pub async fn connect<M, const P: usize, const W: usize, const S: usize>(
//     stack: Stack<'_>,
//     settings: Settings,
//     f: impl AsyncFn(&MessageChannel<'_, M, P, W, S>),
// ) -> Result<(), Error>
// where
//     M: embassy_sync::blocking_mutex::raw::RawMutex,
// {
//     let mut rx_buffer = [0; 1024];
//     let mut tx_buffer = [0; 1024];
//     let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
//     socket.set_timeout(None);

//     let remote_endpoint = (settings.address, settings.port);
//     socket
//         .connect(remote_endpoint)
//         .await
//         .map_err(Error::ConnectError)?;

//     let (mut rx, mut tx) = socket.split();

//     let rx_channel: Channel<M, PacketGeneric<'_, P, W, S>, 1> = Channel::new();
//     let tx_channel: Channel<M, PacketGeneric<'_, P, W, S>, 1> = Channel::new();
//     let rx_channel_sender = rx_channel.sender();
//     let rx_channel_receiver = rx_channel.receiver();
//     let tx_channel_sender = tx_channel.sender();
//     let tx_channel_receiver = tx_channel.receiver();

//     let cancel_pubsub: PubSubChannel<M, ChannelCloseReason, 1, 3, 3> = PubSubChannel::new();
//     let cancel_pub = cancel_pubsub
//         .publisher()
//         .expect("Failed to create cancel_pub");
//     let rx_cancel_pub = cancel_pubsub.publisher().expect("rx_cancel_pub");
//     let tx_cancel_pub = cancel_pubsub.publisher().expect("tx_cancel_pub");
//     let cancel_sub = cancel_pubsub
//         .subscriber()
//         .expect("Failed to create cancel_sub");
//     let mut rx_cancel_sub = cancel_pubsub.subscriber().expect("rx_cancel_sub");
//     let mut tx_cancel_sub = cancel_pubsub.subscriber().expect("tx_cancel_sub");

//     let rx_fut = async {
//         let mut buf = [0; 8];
//         info!("rx future starting");

//         loop {
//             info!("Reading 8 bytes (or cancelling)...");
//             match select(rx.read_exact(&mut buf), rx_cancel_sub.next_message_pure()).await {
//                 Either::First(read) => match read {
//                     Ok(_) => {
//                         info!("Read data {:?}, sending to channel", &buf);
//                         // FIXME: this should select on the cancel channel as well,
//                         // so we can cancel while waiting for a slot to put the message
//                         rx_channel_sender.send(buf).await;
//                         info!("Sent data {:?} to channel", &buf)
//                     }
//                     Err(e) => {
//                         info!("Read error {}, stopping reading", e);
//                         if let Err(e) = rx_cancel_pub.try_publish(true) {
//                             info!("rx_fut failed to cancel - already cancelled '{}'", e);
//                         } else {
//                             info!("rx_fut cancelled");
//                         }
//                         break;
//                     }
//                 },
//                 Either::Second(_) => {
//                     info!("rx_fut cancelled, stopping reading");
//                     break;
//                 }
//             }
//         }
//     };

//     let tx_fut = async {
//         let mut index: u64 = 0;
//         info!("tx future starting");
//         loop {
//             info!("About to send data ({}){:?}", &index, &index.to_ne_bytes());

//             match select(
//                 tx_channel_receiver.receive(),
//                 tx_cancel_sub.next_message_pure(),
//             )
//             .await
//             {
//                 Either::First(write) => match write {
//                     Ok(_) => {
//                         info!("...Sent data ({}){:?}", &index, &index.to_ne_bytes());
//                         index += 1;
//                         Delay.delay_ms(500).await;
//                     }
//                     Err(e) => {
//                         info!("Write error {}, stopping writing", e);
//                         if let Err(e) = tx_cancel_pub.try_publish(true) {
//                             info!("tx_fut failed to cancel - already cancelled '{}'", e);
//                         } else {
//                             info!("tx_fut cancelled");
//                         }
//                         break;
//                     }
//                 },
//                 Either::Second(_) => {
//                     info!("tx_fut cancelled, stopping writing");
//                     break;
//                 }
//             }
//         }
//     };

//     let poll_fut = async {
//         loop {
//             match with_timeout(Duration::from_millis(5000), rx_channel_receiver.receive()).await {
//                 Ok(buf) => info!(
//                     "Polled msg from channel, packet type {:?}",
//                     buf.packet_type()
//                 ),
//                 Err(_) => info!("!!! timeout"),
//             }
//         }
//     };

//     info!("About to start tcp futures");

//     join3(rx_fut, tx_fut, poll_fut).await;

//     info!("rx and tx futures finished");

//     // let channel: MessageChannel<'_, M, P, W, S> = MessageChannel {
//     //     rx_channel_receiver,
//     //     tx_channel_sender,
//     //     cancel_pub,
//     //     cancel_sub,
//     // };
//     // let channel: MessageChannel<'_, M, P, W, S> = MessageChannel {
//     //     rx_channel_receiver: rx_channel.receiver(),
//     //     tx_channel_sender: tx_channel.sender(),
//     //     cancel_pub: cancel_pubsub
//     //         .publisher()
//     //         .expect("Failed to create cancel_pub"),
//     //     cancel_sub: cancel_pubsub
//     //         .subscriber()
//     //         .expect("Failed to create cancel_sub"),
//     // };

//     Ok(())
// }
