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

        // Check first header byte is valid, if not we can error early without
        // trying to read the rest of a packet
        if !PacketType::is_valid_first_header_byte(self.buf[0]) {
            return Err(Error::ConnectionReceiveInvalidData);
        }

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
            return Err(Error::ConnectionReceiveInvalidPacketLength);
        }

        // We have a valid length, decode it
        let remaining_length = {
            let mut r = MqttBufReader::new(&self.buf[1..position]);
            r.get_variable_u32()
                .map_err(|_| Error::ConnectionReceiveInvalidData)?
        } as usize;

        // If packet will not fit in buffer, error
        if position + remaining_length > self.buf.len() {
            return Err(Error::ConnectionReceivePacketBufferOverflow);
        }

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
    use heapless::Vec;

    use crate::{
        codec::mqtt_reader::MqttBufReader,
        data::{
            packet_identifier::PacketIdentifier, property::SubscribeProperty,
            quality_of_service::QualityOfService,
        },
        packets::{
            pingreq::Pingreq,
            subscribe::{Subscribe, SubscriptionRequest},
        },
    };

    use super::*;

    const ENCODED_PINGRESP: [u8; 2] = [0xC0, 0x00];
    const INVALID_PACKET_TYPE: [u8; 2] = [0xC1, 0x00];
    const INVALID_LENGTH: [u8; 5] = [0xC0, 0x80, 0x80, 0x80, 0x80];
    // Packet has first header byte then a length of 16, implying total packet length of 18 (after the first header byte and the single byte length)
    const ENCODED_IMPLIES_PACKET_LENGTH_18: [u8; 2] = [0xC0, 0x10];

    const ENCODED_SUBSCRIBE: [u8; 30] = [
        0x82, 0x1C, 0x15, 0x38, 0x03, 0x0B, 0x80, 0x13, 0x00, 0x0A, 0x74, 0x65, 0x73, 0x74, 0x2f,
        0x74, 0x6f, 0x70, 0x69, 0x63, 0x00, 0x00, 0x06, 0x68, 0x65, 0x68, 0x65, 0x2F, 0x23, 0x01,
    ];

    fn example_subscribe_packet<'a>() -> Subscribe<'a, 16, 16> {
        let primary_request = SubscriptionRequest::new("test/topic", QualityOfService::QoS0);
        let mut additional_requests = Vec::new();
        additional_requests
            .push(SubscriptionRequest::new("hehe/#", QualityOfService::QoS1))
            .unwrap();
        let mut properties = Vec::new();
        properties
            .push(SubscribeProperty::SubscriptionIdentifier(2432.into()))
            .unwrap();
        let packet = Subscribe::new(
            PacketIdentifier(5432),
            primary_request,
            additional_requests,
            properties,
        );

        packet
    }

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

    async fn decode(data: &[u8], packet_generic: PacketGeneric<'_, 16, 16>) {
        let mut write_buf = [];
        let connection = BufferConnection::new(data, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        let packet: PacketGeneric<'_, 16, 16> = client.receive().await.unwrap();

        assert_eq!(packet, packet_generic);
    }

    async fn encode<P: Packet + write::Write>(packet: P, encoded: &[u8]) {
        let read_buf = [];
        let mut write_buf = [0; 1024];
        let connection = BufferConnection::new(&read_buf, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        client.send(packet).await.unwrap();

        assert_eq!(&write_buf[0..encoded.len()], encoded);
    }

    #[tokio::test]
    async fn decode_pingresp() {
        decode(
            &ENCODED_PINGRESP,
            PacketGeneric::Pingreq(Pingreq::default()),
        )
        .await;
    }

    #[tokio::test]
    async fn encode_pingresp() {
        encode(Pingreq::default(), &ENCODED_PINGRESP).await;
    }

    #[tokio::test]
    async fn decode_subscribe() {
        let packet = example_subscribe_packet();
        let packet_generic = PacketGeneric::Subscribe(packet);
        decode(&ENCODED_SUBSCRIBE, packet_generic).await;
    }

    #[tokio::test]
    async fn encode_subscribe() {
        encode(example_subscribe_packet(), &ENCODED_SUBSCRIBE).await;
    }

    #[tokio::test]
    async fn decode_fails_on_invalid_packet_type() {
        let mut write_buf = [];
        let connection = BufferConnection::new(&INVALID_PACKET_TYPE, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        assert_eq!(
            client.receive::<16, 16>().await,
            Err(Error::ConnectionReceiveInvalidData)
        );
    }

    #[tokio::test]
    async fn decode_fails_on_invalid_length_encoding() {
        let mut write_buf = [];
        let connection = BufferConnection::new(&INVALID_LENGTH, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        assert_eq!(
            client.receive::<16, 16>().await,
            Err(Error::ConnectionReceiveInvalidPacketLength)
        );
    }

    #[tokio::test]
    async fn decode_fails_on_packet_length_bigger_than_buffer() {
        let mut write_buf = [];
        let connection = BufferConnection::new(&ENCODED_IMPLIES_PACKET_LENGTH_18, &mut write_buf);

        let mut buf = [0; 17];
        let mut client = PacketClient::new(connection, &mut buf);

        assert_eq!(
            client.receive::<16, 16>().await,
            Err(Error::ConnectionReceivePacketBufferOverflow)
        );
    }
}
