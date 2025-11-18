use core::net::Ipv4Addr;
use embassy_net::{
    tcp::{ConnectError, TcpSocket},
    Stack,
};
use embassy_sync::{
    channel::{self, Channel},
    pubsub::{PubSubChannel, Publisher, Subscriber},
};
use mountain_mqtt::packets::packet_generic::PacketGeneric;

#[derive(Debug, Clone, Copy)]
pub enum ChannelCloseReason {
    Closed,
    ReadError,
    WriteError,
}

pub struct Settings {
    /// The address of the MQTT server
    pub address: Ipv4Addr,

    /// The port of the MQTT server
    pub port: u16,
}

pub enum State {
    Idle,
    Running,
    Stopped,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    ConnectError(ConnectError),
}

// const P: usize = 16;
// const W: usize = 16;
// const S: usize = 16;

pub struct MessageChannel<'a, M, const P: usize, const W: usize, const S: usize>
where
    M: embassy_sync::blocking_mutex::raw::RawMutex,
{
    state: State,
    rx_channel: Channel<M, PacketGeneric<'a, P, W, S>, 1>,
    tx_channel: Channel<M, PacketGeneric<'a, P, W, S>, 1>,
    // rx_channel_receiver: channel::Receiver<'a, M, PacketGeneric<'a, P, W, S>, 1>,
    // tx_channel_sender: channel::Sender<'a, M, PacketGeneric<'a, P, W, S>, 1>,
    // cancel_pub: Publisher<'a, M, ChannelCloseReason, 1, 3, 3>,
    // cancel_sub: Subscriber<'a, M, ChannelCloseReason, 1, 3, 3>,
}

pub struct MessageChannelRunner<'a, M, const P: usize, const W: usize, const S: usize>
where
    M: embassy_sync::blocking_mutex::raw::RawMutex,
{
    running: bool,
    rx_channel_sender: channel::Sender<'a, M, PacketGeneric<'a, P, W, S>, 1>,
    tx_channel_receiver: channel::Receiver<'a, M, PacketGeneric<'a, P, W, S>, 1>,
    // rx_cancel_pub: Publisher<'a, M, ChannelCloseReason, 1, 3, 3>,
    // tx_cancel_pub: Publisher<'a, M, ChannelCloseReason, 1, 3, 3>,
    // rx_cancel_sub: Subscriber<'a, M, ChannelCloseReason, 1, 3, 3>,
    // tx_cancel_sub: Subscriber<'a, M, ChannelCloseReason, 1, 3, 3>,
}

pub async fn connect<'a, M, const P: usize, const W: usize, const S: usize>(
    stack: Stack<'_>,
    settings: Settings,
    f: impl AsyncFn(&str),
) -> Result<
    (
        MessageChannel<'a, M, P, W, S>,
        MessageChannelRunner<'a, M, P, W, S>,
    ),
    Error,
>
where
    M: embassy_sync::blocking_mutex::raw::RawMutex + 'a,
{
    let mut rx_buffer = [0; 1024];
    let mut tx_buffer = [0; 1024];
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(None);

    let remote_endpoint = (settings.address, settings.port);
    socket
        .connect(remote_endpoint)
        .await
        .map_err(Error::ConnectError)?;

    // let rx_channel = Channel::new();
    // let tx_channel = Channel::new();
    // let rx_channel_receiver = rx_channel.receiver();
    // let tx_channel_sender = tx_channel.sender();
    // let cancel_pubsub = PubSubChannel::new();

    let channel: MessageChannel<'a, M, P, W, S> = MessageChannel::new();

    // let channel = MessageChannel {
    //     state: State::Idle,
    //     rx_channel,
    //     tx_channel,
    //     // rx_channel_receiver,
    //     // tx_channel_sender,
    //     // cancel_pub: cancel_pubsub
    //     //     .publisher()
    //     //     .expect("Failed to create cancel_pub"),
    //     // cancel_sub: cancel_pubsub
    //     //     .subscriber()
    //     //     .expect("Failed to create cancel_sub"),
    // };

    let runner: MessageChannelRunner<'a, M, P, W, S> = MessageChannelRunner {
        running: false,
        rx_channel_sender: channel.rx_channel.sender(),
        tx_channel_receiver: channel.tx_channel.receiver(),
        // rx_cancel_pub: cancel_pubsub
        //     .publisher()
        //     .expect("Failed to create rx_cancel_pub"),
        // tx_cancel_pub: cancel_pubsub
        //     .publisher()
        //     .expect("Failed to create tx_cancel_pub"),
        // rx_cancel_sub: cancel_pubsub
        //     .subscriber()
        //     .expect("Failed to create rx_cancel_sub"),
        // tx_cancel_sub: cancel_pubsub
        //     .subscriber()
        //     .expect("Failed to create tx_cancel_sub"),
    };

    Ok((channel, runner))
}

impl<'a, M, const P: usize, const W: usize, const S: usize> MessageChannel<'a, M, P, W, S>
where
    M: embassy_sync::blocking_mutex::raw::RawMutex,
{
    pub fn new() -> Self {
        Self {
            state: State::Idle,
            rx_channel: Channel::new(),
            tx_channel: Channel::new(),
        }
    }
}
