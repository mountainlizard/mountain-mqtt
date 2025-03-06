use core::net::Ipv4Addr;
use std::time::Duration;

use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{
    client::{ClientNoQueue, Delay, EventHandler},
    error::{PacketReadError, PacketWriteError},
    packet_client::Connection,
};

#[derive(Clone)]
pub struct TokioDelay;

impl Delay for TokioDelay {
    async fn delay_us(&mut self, us: u32) {
        tokio::time::sleep(Duration::from_micros(us as u64)).await;
    }
}

pub struct ConnectionTcpStream {
    inner: TcpStream,
}

impl ConnectionTcpStream {
    /// Create a new adapter
    pub fn new(inner: TcpStream) -> Self {
        ConnectionTcpStream { inner }
    }

    /// Consume the adapter, returning the inner object.
    pub fn into_inner(self) -> TcpStream {
        self.inner
    }

    /// Borrow the inner object.
    pub fn inner(&self) -> &TcpStream {
        &self.inner
    }

    /// Mutably borrow the inner object.
    pub fn inner_mut(&mut self) -> &mut TcpStream {
        &mut self.inner
    }
}

impl Connection for ConnectionTcpStream {
    async fn send(&mut self, buf: &[u8]) -> Result<(), PacketWriteError> {
        self.inner
            .write_all(buf)
            .await
            .map_err(|_| PacketWriteError::ConnectionSend)
    }

    async fn receive(&mut self, buf: &mut [u8]) -> Result<(), PacketReadError> {
        self.inner
            .read_exact(buf)
            .await
            .map_err(|_| PacketReadError::ConnectionReceive)?;
        Ok(())
    }

    async fn receive_if_ready(&mut self, buf: &mut [u8]) -> Result<bool, PacketReadError> {
        // Try to read data, this may fail with `WouldBlock`
        // if no data is available
        match self.inner.try_read(buf) {
            // If length is 0, the stream's read half is closed, this is a read error
            // since no data will be read in future
            Ok(0) => Err(PacketReadError::ConnectionReceive),

            // We have read some bytes, we may need to perform more reads to finish
            Ok(n) => {
                if n == buf.len() {
                    Ok(true)
                } else {
                    self.receive(&mut buf[n..]).await?;
                    Ok(true)
                }
            }

            // There is no data at present - not an error, just return false
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),

            // Any other error is a real receive error
            Err(_e) => Err(PacketReadError::ConnectionReceive),
        }
    }
}

pub async fn client_tcp<F, const P: usize>(
    ip: Ipv4Addr,
    port: u16,
    timeout_millis: u32,
    buf: &mut [u8],
    event_handler: F,
) -> ClientNoQueue<'_, ConnectionTcpStream, TokioDelay, F, P>
where
    F: EventHandler<P>,
{
    let addr = core::net::SocketAddr::new(ip.into(), port);
    let tcp_stream = TcpStream::connect(addr).await.unwrap();
    let connection = ConnectionTcpStream::new(tcp_stream);

    let delay = TokioDelay;
    ClientNoQueue::new(connection, buf, delay, timeout_millis, event_handler)
}
