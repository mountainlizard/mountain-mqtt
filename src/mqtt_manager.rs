use crate::client::{Client, ClientError};

/// Identifier for a particular connection. This changes when a reconnection
/// is completed (e.g. if server is unresponsive)
#[derive(Debug, PartialEq, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConnectionId(u32);

impl ConnectionId {
    pub fn new(id: u32) -> ConnectionId {
        ConnectionId(id)
    }
}

/// [MqttOperations] can be performed on an MQTT [Client].
/// Application specific actions must implement this to be used
/// with this manager.
#[allow(async_fn_in_trait)]
pub trait MqttOperations {
    /// Perform the operations
    ///
    /// - `client` - to be used to perform the operations
    /// - `client_id` - the id with which the client connected to the server,
    ///     for example this can be used to customise topics to a particular device
    /// - `connection_id` - the id of the current connection. Some operations may
    ///     apply only to a specific connection (e.g. subscribing) and in this
    ///     case `perform` can simply skip these operations.
    /// - `is_retry` - true if this [MqttOperations] has had `perform` called
    ///     previously (resulting in an error and reconnection). It's up to
    ///     the implementation to decide which (if any) operations to repeat
    ///     in this case. Generally it would make sense to repeat publish
    ///     operations that use quality of service 1 (at least once), since
    ///     these messages are intended to be repeated if they fail, and it's
    ///     accepted that they might be received multiple times (e.g. if the
    ///     error that caused a retry was a timeout in acknowledging the message,
    ///     which can occur even if the server received it). Generally QoS0
    ///     messges would not be repeated since they are expected to be received
    ///     "at most once". QoS2 messages are not supported.
    async fn perform<'a, 'b, C>(
        &'b mut self,
        client: &mut C,
        client_id: &'a str,
        connection_id: ConnectionId,
        is_retry: bool,
    ) -> Result<(), ClientError>
    where
        C: Client<'a>;
}
