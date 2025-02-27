use crate::{
    codec::{
        mqtt_reader::{MqttBufReader, MqttReader},
        mqtt_writer::{MqttBufWriter, MqttWriter},
        write,
    },
    data::packet_type::PacketType,
    error::{PacketReadError, PacketWriteError},
    packets::{packet::Packet, packet_generic::PacketGeneric},
};

#[allow(async_fn_in_trait)]
pub trait Connection {
    // Send and flush all data in `buf`
    async fn send(&mut self, buf: &[u8]) -> Result<(), PacketWriteError>;

    // Receive into buffer, waiting to fill it. This may need to await more data.
    async fn receive(&mut self, buf: &mut [u8]) -> Result<(), PacketReadError>;

    /// If no data at all is ready, then return immediately with `Ok(false)`,
    /// leaving the underlying stream and `buf` unaltered.
    /// If any data is ready  then receive into buffer, waiting to fill it,
    /// then return `Ok(true)`.
    /// Note that this method can still await data, because it is only required
    /// to return `Ok(false)` in the case where no data at all is ready. If less
    /// data is available than `buf.len()`, the method may still proceed, reading
    /// the available data and then waiting for more to fill `buf`.
    /// This is fairly well suited to MQTT packets - first call `receive_if_ready`
    /// with a `buf` of length 1 - if this returns `Ok(false)` then sleep for
    /// a reasonable interval and try again. If it returns `Ok(true)`, continue
    /// reading a whole packet using `receive`, using larger buffers if possible.
    /// While this may lead to an indefinite await for the rest of the packet, this
    /// should only occur in the case of a malicious server or poor connection, not
    /// in the most common case where there is a long gap between incoming packets,
    /// but once the first byte of a packet is received the rest is received quickly
    /// afterwards.
    /// This approach is used to make it easier to use a variety of underlying streams,
    /// since we only need something like embedded-async's `ReadReady` trait, or
    /// tokio's `TCPStream.try_read`
    /// More sophisticated approaches are definitely possible.
    async fn receive_if_ready(&mut self, buf: &mut [u8]) -> Result<bool, PacketReadError>;
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

    pub async fn send<P>(&mut self, packet: P) -> Result<(), PacketWriteError>
    where
        P: Packet + write::Write,
    {
        let len = {
            let mut r = MqttBufWriter::new(self.buf);
            r.put(&packet)?;
            r.position()
        };
        self.connection.send(&self.buf[0..len]).await
    }

    pub async fn receive<const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
    ) -> Result<PacketGeneric<'_, PROPERTIES_N, REQUEST_N>, PacketReadError> {
        // First, try to read one byte with blocking
        self.connection.receive(&mut self.buf[0..1]).await?;

        // Then the rest of the packet
        self.receive_rest_of_packet().await
    }

    pub async fn receive_if_ready<const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
    ) -> Result<Option<PacketGeneric<'_, PROPERTIES_N, REQUEST_N>>, PacketReadError> {
        // First, try to read one byte without blocking - if this returns false, no packet is ready
        // and we can return immediately to avoid blocking
        let packet_started = self
            .connection
            .receive_if_ready(&mut self.buf[0..1])
            .await?;

        if !packet_started {
            return Ok(None);
        }

        // We have packet, and its first byte is in our buffer, so receive the rest of the packet
        let packet = self.receive_rest_of_packet().await?;
        Ok(Some(packet))
    }

    async fn receive_rest_of_packet<const PROPERTIES_N: usize, const REQUEST_N: usize>(
        &mut self,
    ) -> Result<PacketGeneric<'_, PROPERTIES_N, REQUEST_N>, PacketReadError> {
        let mut position: usize = 1;

        // Check first header byte is valid, if not we can error early without
        // trying to read the rest of a packet
        if !PacketType::is_valid_first_header_byte(self.buf[0]) {
            return Err(PacketReadError::InvalidPacketType);
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
            return Err(PacketReadError::InvalidVariableByteIntegerEncoding);
        }

        // We have a valid length, decode it
        let remaining_length = {
            let mut r = MqttBufReader::new(&self.buf[1..position]);
            r.get_variable_u32()?
        } as usize;

        // If packet will not fit in buffer, error
        if position + remaining_length > self.buf.len() {
            return Err(PacketReadError::PacketTooLargeForBuffer);
        }

        // Read the rest of the packet
        self.connection
            .receive(&mut self.buf[position..position + remaining_length])
            .await?;
        position += remaining_length;

        // We can now decode the packet from the buffer
        let packet_buf = &mut self.buf[0..position];
        let mut packet_reader = MqttBufReader::new(packet_buf);
        let packet_generic = packet_reader.get()?;

        Ok(packet_generic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        codec::mqtt_reader::MqttBufReader,
        data::{
            packet_identifier::PacketIdentifier,
            property::{ConnectProperty, SubscribeProperty},
            quality_of_service::QualityOfService,
        },
        packets::{
            connect::Connect,
            pingreq::Pingreq,
            subscribe::{Subscribe, SubscriptionRequest},
        },
    };
    use heapless::Vec;

    const ENCODED_PINGRESP: [u8; 2] = [0xC0, 0x00];
    const INVALID_PACKET_TYPE: [u8; 2] = [0xC1, 0x00];
    const INVALID_LENGTH: [u8; 5] = [0xC0, 0x80, 0x80, 0x80, 0x80];
    // Packet has first header byte then a length of 16, implying total packet length of 18 (after the first header byte and the single byte length)
    const ENCODED_IMPLIES_PACKET_LENGTH_18: [u8; 2] = [0xC0, 0x10];

    const ENCODED_SUBSCRIBE: [u8; 30] = [
        0x82, 0x1C, 0x15, 0x38, 0x03, 0x0B, 0x80, 0x13, 0x00, 0x0A, 0x74, 0x65, 0x73, 0x74, 0x2f,
        0x74, 0x6f, 0x70, 0x69, 0x63, 0x00, 0x00, 0x06, 0x68, 0x65, 0x68, 0x65, 0x2F, 0x23, 0x01,
    ];

    const ENCODED_CONNECT: [u8; 18] = [
        0x10, 0x10, 0x00, 0x04, 0x4d, 0x51, 0x54, 0x54, 0x05, 0x02, 0x00, 0x3c, 0x03, 0x21, 0x00,
        0x14, 0x00, 0x00,
    ];

    // Copy of valid ENCODED_CONNECT above, except that it has a "remaining length" in the
    // header byte that is 1 byte too long, and so should produce an incorrect packet length error. Note that we need
    // to also add a padding byte to the data so that the client can attempt to read the whole expected packet buffer and get
    // as far as trying to then decode it and encounter a mismatch in the packet_generic Read implementation
    const ENCODED_CONNECT_INCORRECT_PACKET_LENGTH: [u8; 19] = [
        0x10, 0x11, 0x00, 0x04, 0x4d, 0x51, 0x54, 0x54, 0x05, 0x02, 0x00, 0x3c, 0x03, 0x21, 0x00,
        0x14, 0x00, 0x00, 0x00,
    ];

    fn example_subscribe_packet<'a>() -> Subscribe<'a, 16, 16> {
        let primary_request = SubscriptionRequest::new("test/topic", QualityOfService::Qos0);
        let mut additional_requests = Vec::new();
        additional_requests
            .push(SubscriptionRequest::new("hehe/#", QualityOfService::Qos1))
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

    fn example_connect_packet<'a>() -> Connect<'a, 16> {
        let mut packet = Connect::new(60, None, None, "", true, None, Vec::new());
        packet
            .properties
            .push(ConnectProperty::ReceiveMaximum(20.into()))
            .unwrap();
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
        async fn send(&mut self, buf: &[u8]) -> Result<(), PacketWriteError> {
            self.writer.put_slice(buf)
        }

        async fn receive(&mut self, buf: &mut [u8]) -> Result<(), PacketReadError> {
            let slice = self.reader.get_slice(buf.len())?;
            buf.copy_from_slice(slice);
            Ok(())
        }

        async fn receive_if_ready(&mut self, buf: &mut [u8]) -> Result<bool, PacketReadError> {
            // Assume data is always ready, will error on underflow as required for tests
            self.receive(buf).await?;
            Ok(true)
        }
    }

    async fn decode(data: &[u8], packet_generic: PacketGeneric<'_, 16, 16>) {
        let mut write_buf = [];
        let connection = BufferConnection::new(data, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        let packet: Option<PacketGeneric<'_, 16, 16>> = client.receive_if_ready().await.unwrap();

        assert_eq!(packet, Some(packet_generic));
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
    async fn error_on_decode_connect_with_incorrect_length() {
        let mut write_buf = [];
        let connection =
            BufferConnection::new(&ENCODED_CONNECT_INCORRECT_PACKET_LENGTH, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        let packet: Result<Option<PacketGeneric<'_, 16, 16>>, PacketReadError> =
            client.receive_if_ready().await;
        assert_eq!(packet, Err(PacketReadError::IncorrectPacketLength));
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
    async fn decode_connect() {
        decode(
            &ENCODED_CONNECT,
            PacketGeneric::Connect(example_connect_packet()),
        )
        .await;
    }

    #[tokio::test]
    async fn encode_connect() {
        encode(example_connect_packet(), &ENCODED_CONNECT).await;
    }

    #[tokio::test]
    async fn decode_fails_on_invalid_packet_type() {
        let mut write_buf = [];
        let connection = BufferConnection::new(&INVALID_PACKET_TYPE, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        assert_eq!(
            client.receive_if_ready::<16, 16>().await,
            Err(PacketReadError::InvalidPacketType)
        );
    }

    #[tokio::test]
    async fn decode_fails_on_invalid_length_encoding() {
        let mut write_buf = [];
        let connection = BufferConnection::new(&INVALID_LENGTH, &mut write_buf);

        let mut buf = [0; 1024];
        let mut client = PacketClient::new(connection, &mut buf);

        assert_eq!(
            client.receive_if_ready::<16, 16>().await,
            Err(PacketReadError::InvalidVariableByteIntegerEncoding)
        );
    }

    #[tokio::test]
    async fn decode_fails_on_packet_length_bigger_than_buffer() {
        let mut write_buf = [];
        let connection = BufferConnection::new(&ENCODED_IMPLIES_PACKET_LENGTH_18, &mut write_buf);

        let mut buf = [0; 17];
        let mut client = PacketClient::new(connection, &mut buf);

        assert_eq!(
            client.receive_if_ready::<16, 16>().await,
            Err(PacketReadError::PacketTooLargeForBuffer)
        );
    }
}
