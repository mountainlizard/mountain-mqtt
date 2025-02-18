use embedded_io::ReadReady;
use embedded_io_async::{Read, Write};

use crate::{
    error::{PacketReadError, PacketWriteError},
    packet_client::Connection,
};

/// Contains an instance of T with [Read], [Write] and [ReadReady].
/// We implement [Connection] for this rather than T directly to avoid
/// issues with conflicting implementations of external types T that
/// could in future add [Read], [Write] and [ReadReady]
pub struct ConnectionEmbedded<T>
where
    T: Read + Write + ReadReady,
{
    inner: T,
}

impl<T> ConnectionEmbedded<T>
where
    T: Read + Write + ReadReady,
{
    /// Create a new adapter
    pub fn new(inner: T) -> Self {
        ConnectionEmbedded { inner }
    }

    /// Consume the adapter, returning the inner object.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Borrow the inner object.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Mutably borrow the inner object.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Connection for ConnectionEmbedded<T>
where
    T: Read + Write + ReadReady,
{
    async fn send(&mut self, buf: &[u8]) -> Result<(), PacketWriteError> {
        self.inner
            .write_all(buf)
            .await
            .map_err(|_| PacketWriteError::ConnectionSend)?;
        self.inner
            .flush()
            .await
            .map_err(|_| PacketWriteError::ConnectionSend)
    }

    async fn receive(&mut self, buf: &mut [u8]) -> Result<(), PacketReadError> {
        self.inner
            .read_exact(buf)
            .await
            .map_err(|_| PacketReadError::ConnectionReceive)
    }

    async fn receive_if_ready(&mut self, buf: &mut [u8]) -> Result<bool, PacketReadError> {
        if self
            .inner
            .read_ready()
            .map_err(|_| PacketReadError::ConnectionReceive)?
        {
            self.receive(buf).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
