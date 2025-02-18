use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{
    error::{PacketReadError, PacketWriteError},
    packet_client::Connection,
};

impl Connection for TcpStream {
    async fn send(&mut self, buf: &[u8]) -> Result<(), PacketWriteError> {
        self.write_all(buf)
            .await
            .map_err(|_| PacketWriteError::ConnectionSend)
    }

    async fn receive(&mut self, buf: &mut [u8]) -> Result<(), PacketReadError> {
        self.read_exact(buf)
            .await
            .map_err(|_| PacketReadError::ConnectionReceive)?;
        Ok(())
    }

    async fn receive_if_ready(&mut self, buf: &mut [u8]) -> Result<bool, PacketReadError> {
        // Try to read data, this may fail with `WouldBlock`
        // if no data is available
        match self.try_read(buf) {
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
