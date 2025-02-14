use embedded_io::ReadReady;
use embedded_io_async::{Read, Write};

use crate::{
    codec::{
        mqtt_reader::{MqttBufReader, MqttReader},
        mqtt_writer::{MqttBufWriter, MqttWriter},
    },
    data::packet_type::PacketType,
    error::Error,
    packets::{
        packet::{Packet, PacketWrite},
        packet_generic::PacketGeneric,
    },
};

// TODO: review this
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
        P: Packet + PacketWrite,
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

        let packet_type: PacketType = self.buf[0]
            .try_into()
            .map_err(|_| Error::ConnectionReceiveInvalidData)?;

        // Read up to 4 bytes into buffer as variable u32
        // First byte always exists
        self.connection
            .receive(&mut self.buf[position..position + 1])
            .await?;
        position += 1;

        // Read bytes until we see the last one or run out
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

        // We didn't see the end of the length
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
        let packet_generic = match packet_type {
            PacketType::Connect => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Connect(packet)
            }
            PacketType::Connack => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Connack(packet)
            }
            PacketType::Publish => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Publish(packet)
            }
            PacketType::Puback => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Puback(packet)
            }
            PacketType::Pubrec => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Pubrec(packet)
            }
            PacketType::Pubrel => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Pubrel(packet)
            }
            PacketType::Pubcomp => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Pubcomp(packet)
            }
            PacketType::Subscribe => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Subscribe(packet)
            }
            PacketType::Suback => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Suback(packet)
            }
            PacketType::Unsubscribe => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Unsubscribe(packet)
            }
            PacketType::Unsuback => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Unsuback(packet)
            }
            PacketType::Pingreq => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Pingreq(packet)
            }
            PacketType::Pingresp => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Pingresp(packet)
            }
            PacketType::Disconnect => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Disconnect(packet)
            }
            PacketType::Auth => {
                let packet = packet_reader.get().map_err(|_| Error::MalformedPacket)?;
                PacketGeneric::Auth(packet)
            }
        };

        Ok(packet_generic)
    }
}
