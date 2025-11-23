use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::{Receiver, Sender},
};
use heapless::Vec;
use mountain_mqtt::{
    client::{ClientError, ConnectionSettings},
    client_state::{ClientState, ClientStateNoQueue},
    data::property::ConnectProperty,
    packets::connect::{Connect, Will},
};

use crate::{packet_bin::PacketBin, raw_client::RawClient};

pub struct PollClient<'a, M, const N: usize>
where
    M: RawMutex,
{
    client_state: ClientStateNoQueue,
    raw_client: RawClient<'a, M, N>,
}

impl<'a, M, const N: usize> PollClient<'a, M, N>
where
    M: RawMutex,
{
    pub fn new(
        sender: Sender<'a, M, PacketBin<N>, 1>,
        receiver: Receiver<'a, M, PacketBin<N>, 1>,
    ) -> Self {
        Self {
            client_state: ClientStateNoQueue::default(),
            raw_client: RawClient::new(sender, receiver),
        }
    }

    pub async fn connect_with_will<const W: usize>(
        &mut self,
        settings: &ConnectionSettings<'_>,
        will: Option<Will<'_, W>>,
    ) -> Result<(), ClientError> {
        let mut properties = Vec::new();
        // By setting maximum topic alias to 0, we prevent the server
        // trying to use aliases, which we don't support. They are optional
        // and only provide for reduced packet size, but would require storing
        // topic names from the server for the length of the connection,
        // which might be awkward without alloc.
        properties
            .push(ConnectProperty::TopicAliasMaximum(0.into()))
            .unwrap();
        let packet: Connect<'_, 1, W> = Connect::new(
            settings.keep_alive(),
            *settings.username(),
            *settings.password(),
            settings.client_id(),
            true,
            will,
            properties,
        );
        self.client_state.connect(&packet)?;
        self.raw_client.send(packet).await
    }

    pub async fn connect(&mut self, settings: &ConnectionSettings<'_>) -> Result<(), ClientError> {
        self.connect_with_will::<0>(settings, None).await
    }
}
