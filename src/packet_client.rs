use embedded_io::ReadReady;
use embedded_io_async::{Read, Write};

use crate::{
    codec::{
        mqtt_reader::{MqttBufReader, MqttReader},
        mqtt_writer::{MqttBufWriter, MqttWriter},
        write,
    },
    data::packet_type::PacketType,
    error::Error,
    packets::{packet::Packet, packet_generic::PacketGeneric},
};

#[allow(async_fn_in_trait)]
pub trait Connection {
    // Send and flush all data in `buf`
    async fn send(&mut self, buf: &[u8]) -> Result<(), Error>;

    // Receive into buffer, waiting to fill it
    async fn receive(&mut self, buf: &mut [u8]) -> Result<(), Error>;
}

pub trait ConnectionReady: Connection {
    /// Check whether the TCP connection is ready to provide any data
    fn receive_ready(&mut self) -> Result<bool, Error>;
}

impl<T> Connection for T
where
    T: Read + Write,
{
    async fn send(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.write_all(buf)
            .await
            .map_err(|_| Error::ConnectionSend)?;
        self.flush().await.map_err(|_| Error::ConnectionSend)
    }

    async fn receive(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        self.read_exact(buf)
            .await
            .map_err(|_| Error::ConnectionReceive)
    }
}

impl<T> ConnectionReady for T
where
    T: Read + Write + ReadReady,
{
    fn receive_ready(&mut self) -> Result<bool, Error> {
        self.read_ready().map_err(|_| Error::ConnectionReceive)
    }
}

pub struct PacketClient<'a, C> {
    connection: C,
    buf: &'a mut [u8],
}

// struct PositionBuf<'a> {}

impl<'a, C> PacketClient<'a, C>
where
    C: Connection,
{
    pub fn new(connection: C, buf: &'a mut [u8]) -> Self {
        Self { connection, buf }
    }

    pub async fn send<P>(&mut self, packet: P) -> Result<(), Error>
    where
        P: Packet + write::Write,
    {
        let len = {
            let mut r = MqttBufWriter::new(self.buf);
            // TODO: make error more specific - it's more likely to be buffer overrun
            r.put(&packet).map_err(|_| Error::MalformedPacket)?;
            r.position()
        };
        self.connection.send(&self.buf[0..len]).await
    }

    pub async fn receive<const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
    ) -> Result<PacketGeneric<'_, PROPERTIES_N, REQUEST_N>, Error> {
        let mut position: usize = 0;

        // First byte must always be a valid packet type
        self.connection
            .receive(&mut self.buf[position..position + 1])
            .await?;
        position += 1;

        // Check we can parse the packet type, so we can error early on invalid data
        // without trying to read the rest of a packet
        let _packet_type: PacketType = self.buf[0]
            .try_into()
            .map_err(|_| Error::ConnectionReceiveInvalidData)?;

        // Read up to 4 bytes into buffer as variable u32
        // First byte always exists
        self.connection
            .receive(&mut self.buf[position..position + 1])
            .await?;
        position += 1;

        // Read up to 3 more bytes looking for the end of the encoded length
        for _extra in 0..3 {
            if self.buf[position - 1] & 128 == 0 {
                break;
            } else {
                self.connection
                    .receive(&mut self.buf[position..position + 1])
                    .await?;
                position += 1;
            }
        }

        // Error if we didn't see the end of the length
        if self.buf[position - 1] & 128 != 0 {
            return Err(Error::ConnectionReceiveInvalidData);
        }

        // We have a valid length, decode it
        let remaining_length = {
            let mut r = MqttBufReader::new(&self.buf[1..position]);
            r.get_variable_u32()
                .map_err(|_| Error::ConnectionReceiveInvalidData)?
        } as usize;

        // Read the rest of the packet
        self.connection
            .receive(&mut self.buf[position..position + remaining_length])
            .await?;
        position += remaining_length;

        // We can now decode the packet from the buffer
        let packet_buf = &mut self.buf[0..position];
        let mut packet_reader = MqttBufReader::new(packet_buf);
        let packet_generic = packet_reader.get().map_err(|_| Error::MalformedPacket)?;

        Ok(packet_generic)
    }
}

impl<C> PacketClient<'_, C>
where
    C: ConnectionReady,
{
    pub async fn receive_if_ready<const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
    ) -> Result<Option<PacketGeneric<'_, PROPERTIES_N, REQUEST_N>>, Error> {
        if !self.connection.receive_ready()? {
            Ok(None)
        } else {
            self.receive().await.map(Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{codec::mqtt_reader::MqttBufReader, packets::pingreq::Pingreq};

    use super::*;

    const ENCODED: [u8; 2] = [0xC0, 0x00];

    struct BufferConnection<'a> {
        reader: MqttBufReader<'a>,
        writer: MqttBufWriter<'a>,
    }

    impl<'a> BufferConnection<'a> {
        pub fn new(read_buf: &'a [u8], write_buf: &'a mut [u8]) -> Self {
            let reader = MqttBufReader::new(read_buf);
            let writer = MqttBufWriter::new(write_buf);
            BufferConnection { reader, writer }
        }
    }

    impl Connection for BufferConnection<'_> {
        async fn send(&mut self, buf: &[u8]) -> Result<(), Error> {
            self.writer
                .put_slice(buf)
                .map_err(|_| Error::ConnectionSend)
        }

        async fn receive(&mut self, buf: &mut [u8]) -> Result<(), Error> {
            let slice = self
                .reader
                .get_slice(buf.len())
                .map_err(|_| Error::ConnectionReceive)?;
            buf.copy_from_slice(slice);
            Ok(())
        }
    }

    #[tokio::test]
    async fn decode() {
        let mut write_buf = [];
        let connection = BufferConnection::new(&ENCODED, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        let packet: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();

        assert_eq!(packet, PacketGeneric::Pingreq(Pingreq::default()));
    }

    #[tokio::test]
    async fn encode() {
        let read_buf = [];
        let mut write_buf = [0; 1024];
        let connection = BufferConnection::new(&read_buf, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        client.send(Pingreq::default()).await.unwrap();

        assert_eq!(write_buf[0..ENCODED.len()], ENCODED);
    }

    // TODO: tests for failing on invalid packet type (first byte) and invalid length (doesn't terminate in 4 bytes)
}
