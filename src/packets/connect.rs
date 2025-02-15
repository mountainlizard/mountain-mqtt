use super::packet::{Packet, PacketRead, PacketWrite, PROTOCOL_NAME, PROTOCOL_VERSION_5};
use crate::codec::{
    mqtt_reader::MqttReaderError,
    mqtt_writer::{self, MqttWriter},
};
use crate::data::{
    packet_type::PacketType,
    property::{ConnectProperty, WillProperty},
    quality_of_service::QualityOfService,
};
use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Will<'a, const PROPERTIES_N: usize> {
    qos: QualityOfService,
    retain: bool,
    topic_name: &'a str,
    payload: &'a [u8],
    properties: Vec<WillProperty<'a>, PROPERTIES_N>,
}

const CLEAN_START_BIT: u8 = 1 << 1;
const WILL_PRESENT_BIT: u8 = 1 << 2;
const WILL_QOS_SHIFT: i32 = 3;
const WILL_QOS_MASK: u8 = 0x03;
const WILL_RETAIN_BIT: u8 = 1 << 5;
const PASSWORD_PRESENT_BIT: u8 = 1 << 6;
const USERNAME_PRESENT_BIT: u8 = 1 << 7;

#[derive(Debug, PartialEq)]
pub struct Connect<'a, const PROPERTIES_N: usize> {
    keep_alive: u16,
    username: Option<&'a str>,
    password: Option<&'a [u8]>,
    client_id: &'a str,
    clean_start: bool,
    will: Option<Will<'a, PROPERTIES_N>>,
    properties: Vec<ConnectProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize> Connect<'a, PROPERTIES_N> {
    pub fn new(
        keep_alive: u16,
        username: Option<&'a str>,
        password: Option<&'a [u8]>,
        client_id: &'a str,
        clean_start: bool,
        will: Option<Will<'a, PROPERTIES_N>>,
        properties: Vec<ConnectProperty<'a>, PROPERTIES_N>,
    ) -> Self {
        Self {
            keep_alive,
            username,
            password,
            client_id,
            clean_start,
            will,
            properties,
        }
    }

    fn connect_flags(&self) -> u8 {
        let mut flags = 0u8;
        // Note bit 0 is reserved, must be left as 0 (MQTT-3.1.2-2)
        if self.clean_start {
            flags |= CLEAN_START_BIT;
        }
        if let Some(ref will) = self.will {
            flags |= WILL_PRESENT_BIT; // Will present
            flags |= (will.qos as u8) << WILL_QOS_SHIFT;
            if will.retain {
                flags |= WILL_RETAIN_BIT;
            }
        }
        if self.password.is_some() {
            flags |= PASSWORD_PRESENT_BIT;
        }
        if self.username.is_some() {
            flags |= USERNAME_PRESENT_BIT;
        }
        flags
    }
}

impl<const PROPERTIES_N: usize> Packet for Connect<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Connect
    }
}

impl<const PROPERTIES_N: usize> PacketWrite for Connect<'_, PROPERTIES_N> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:

        // Write the fixed parts of the variable header
        writer.put_str(PROTOCOL_NAME)?; // 3.1.2.1 Protocol name
        writer.put_u8(PROTOCOL_VERSION_5)?; // 3.1.2.2 Protocol Version
        writer.put_u8(self.connect_flags())?; // 3.1.2.3 Connect Flags
        writer.put_u16(self.keep_alive)?; // 3.1.2.10 Keep Alive

        // Write the properties vec (3.1.2.11)
        writer.put_variable_u32_delimited_vec(&self.properties)?;

        // Payload:
        // 3.1.3.1 Client Identifier (ClientID)
        writer.put_str(self.client_id)?;

        // Will
        if let Some(ref will) = self.will {
            writer.put_variable_u32_delimited_vec(&will.properties)?; // 3.1.3.2 Will Properties
            writer.put_str(will.topic_name)?; // 3.1.3.3 Will Topic
            writer.put_binary_data(will.payload)?; // 3.1.3.4 Will Payload
        }

        // 3.1.3.5 User Name
        if let Some(username) = self.username {
            writer.put_str(username)?;
        }

        // 3.1.3.6 Password
        if let Some(password) = self.password {
            writer.put_binary_data(password)?;
        }

        Ok(())
    }
}

impl<'a, const PROPERTIES_N: usize> PacketRead<'a> for Connect<'a, PROPERTIES_N> {
    fn get_variable_header_and_payload<R: crate::codec::mqtt_reader::MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        _len: usize,
    ) -> crate::codec::mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // Variable header:

        // Read the fixed parts of the variable header
        let protocol_name = reader.get_str()?; // 3.1.2.1 Protocol name
        let protocol_version = reader.get_u8()?; // 3.1.2.2 Protocol Version

        // See 3.1.2.1 and 3.1.2.2
        if protocol_name != PROTOCOL_NAME || protocol_version != PROTOCOL_VERSION_5 {
            return Err(MqttReaderError::UnsupportedProtocolVersion);
        }

        let connect_flags = reader.get_u8()?; // 3.1.2.3 Connect Flags

        // Check MQTT-3.1.2-2 (connect flags bit 0 must be 0)
        if connect_flags & 0x01 != 0 {
            return Err(MqttReaderError::MalformedPacket);
        }
        let clean_start = connect_flags & (CLEAN_START_BIT) != 0;

        let keep_alive = reader.get_u16()?; // 3.1.2.10 Keep Alive

        // Read the properties vec (3.1.2.11)
        let mut properties = Vec::new();
        reader.get_property_list(&mut properties)?;

        // Payload:
        // 3.1.3.1 Client Identifier (ClientID)
        let client_id = reader.get_str()?;

        // Will
        let has_will = connect_flags & (WILL_PRESENT_BIT) != 0;
        let will = if has_will {
            let will_qos_value = (connect_flags >> WILL_QOS_SHIFT) & WILL_QOS_MASK;
            let will_qos = will_qos_value.try_into()?;
            let will_retain = connect_flags & (WILL_RETAIN_BIT) != 0;

            let mut will_properties = Vec::new();
            reader.get_property_list(&mut will_properties)?; // 3.1.3.2 Will Properties
            let will_topic_name = reader.get_str()?; // 3.1.3.3 Will Topic
            let will_payload = reader.get_binary_data()?; // 3.1.3.4 Will Payload

            Some(Will {
                qos: will_qos,
                retain: will_retain,
                topic_name: will_topic_name,
                payload: will_payload,
                properties: will_properties,
            })
        } else {
            None
        };

        // 3.1.3.5 User Name
        let has_username = connect_flags & (USERNAME_PRESENT_BIT) != 0;
        let username = if has_username {
            Some(reader.get_str()?)
        } else {
            None
        };

        // 3.1.3.6 Password
        let has_password = connect_flags & (PASSWORD_PRESENT_BIT) != 0;
        let password = if has_password {
            Some(reader.get_binary_data()?)
        } else {
            None
        };

        let packet = Connect::new(
            keep_alive,
            username,
            password,
            client_id,
            clean_start,
            will,
            properties,
        );
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{
        mqtt_reader::{MqttBufReader, MqttReader},
        mqtt_writer::MqttBufWriter,
        write::Write,
    };

    use super::*;

    fn example_packet<'a>() -> Connect<'a, 1> {
        let mut packet = Connect::new(60, None, None, "", true, None, Vec::new());
        packet
            .properties
            .push(ConnectProperty::ReceiveMaximum(20.into()))
            .unwrap();
        packet
    }

    const EXAMPLE_DATA: [u8; 18] = [
        0x10, 0x10, 0x00, 0x04, 0x4d, 0x51, 0x54, 0x54, 0x05, 0x02, 0x00, 0x3c, 0x03, 0x21, 0x00,
        0x14, 0x00, 0x00,
    ];

    fn example_packet_will<'a>() -> Connect<'a, 1> {
        let mut will_properties = Vec::new();
        will_properties
            .push(WillProperty::MessageExpiryInterval(12345.into()))
            .unwrap();
        let will = Will {
            qos: QualityOfService::QoS2,
            retain: true,
            topic_name: "wt",
            payload: &[1, 2, 3],
            properties: will_properties,
        };
        let mut packet = Connect::new(60, None, None, "", true, Some(will), Vec::new());
        packet
            .properties
            .push(ConnectProperty::ReceiveMaximum(20.into()))
            .unwrap();
        packet
    }

    #[rustfmt::skip]
    const EXAMPLE_DATA_WILL: [u8; 33] = [
        // header byte
        0x10,
        // packet length
        0x1F,
        // protocol name and version
        0x00, 0x04, 0x4d, 0x51, 0x54, 0x54, 0x05,
        // Connect flags, bit 0 reserved as 0, bit 1 clean start, bit 2 has will, bit 3+4 will QoS (2),
        // bit 5 will retain, bit 6 password, bit 7 username
        0b0011_0110,
        // Keep alive
        0x00, 0x3c,
        // length of encoded properties
        0x03,
        // Receive maximum id 0x21, contents
        0x21, 0x00, 0x14,
        // Client id (length 0, no data)
        0x00, 0x00,
        // Will properties
        // length of encoded properties
        0x05,
        // Property: Message expiry interval id 0x02, u32 value
        0x02, 0x00, 0x00, 0x30, 0x39,
        // Will topic
        0x00, 0x02, 0x77, 0x74,
        // Will payload
        0x00, 0x03, 0x01, 0x02, 0x03,
    ];

    fn example_packet_will2<'a>() -> Connect<'a, 1> {
        let mut will_properties = Vec::new();
        will_properties
            .push(WillProperty::MessageExpiryInterval(12345.into()))
            .unwrap();
        let will = Will {
            qos: QualityOfService::QoS1,
            retain: false,
            topic_name: "tw",
            payload: &[3, 2, 1],
            properties: will_properties,
        };
        let mut packet = Connect::new(
            60,
            Some("user1"),
            Some(&[0x42, 0x84]),
            "",
            false,
            Some(will),
            Vec::new(),
        );
        packet
            .properties
            .push(ConnectProperty::ReceiveMaximum(20.into()))
            .unwrap();
        packet
    }

    #[rustfmt::skip]
    const EXAMPLE_DATA_WILL2: [u8; 44] = [
        // header byte
        0x10,
        // packet length
        0x2A,
        // protocol name and version
        0x00, 0x04, 0x4d, 0x51, 0x54, 0x54, 0x05,
        // Connect flags, bit 0 reserved as 0, bit 1 clean start, bit 2 has will, bit 3+4 will QoS (1),
        // bit 5 will retain, bit 6 password, bit 7 username
        0b1100_1100,
        // Keep alive
        0x00, 0x3c,
        // length of encoded properties
        0x03,
        // Receive maximum id 0x21, contents
        0x21, 0x00, 0x14,
        // Client id (length 0, no data)
        0x00, 0x00,
        // Will properties
        // length of encoded properties
        0x05,
        // Property: Message expiry interval id 0x02, u32 value
        0x02, 0x00, 0x00, 0x30, 0x39,
        // Will topic
        0x00, 0x02, 0x74, 0x77,
        // Will payload
        0x00, 0x03, 0x03, 0x02, 0x01,
        // User name
        0x00, 0x05, 0x75, 0x73, 0x65, 0x72, 0x31,
        // Password
        0x00, 0x02, 0x42,0x84
    ];

    fn example_packet_username<'a>() -> Connect<'a, 1> {
        let mut packet = Connect::new(60, Some("user"), None, "", true, None, Vec::new());
        packet
            .properties
            .push(ConnectProperty::ReceiveMaximum(20.into()))
            .unwrap();
        packet
    }

    const EXAMPLE_DATA_USERNAME: [u8; 24] = [
        0x10, 0x16, 0x00, 0x04, 0x4d, 0x51, 0x54, 0x54, 0x05, 0x82, 0x00, 0x3c, 0x03, 0x21, 0x00,
        0x14, 0x00, 0x00, 0x00, 0x04, 0x75, 0x73, 0x65, 0x72,
    ];

    fn example_packet_username_password<'a>() -> Connect<'a, 1> {
        let mut packet = Connect::new(
            60,
            Some("user"),
            Some("pass".as_bytes()),
            "",
            true,
            None,
            Vec::new(),
        );
        packet
            .properties
            .push(ConnectProperty::ReceiveMaximum(20.into()))
            .unwrap();
        packet
    }

    const EXAMPLE_DATA_USERNAME_PASSWORD: [u8; 30] = [
        0x10, 0x1C, 0x00, 0x04, 0x4d, 0x51, 0x54, 0x54, 0x05, 0xC2, 0x00, 0x3c, 0x03, 0x21, 0x00,
        0x14, 0x00, 0x00, 0x00, 0x04, 0x75, 0x73, 0x65, 0x72, 0x00, 0x04, 0x70, 0x61, 0x73, 0x73,
    ];

    fn example_packet_clientid_username_password<'a>() -> Connect<'a, 1> {
        let mut packet = Connect::new(
            60,
            Some("user"),
            Some("pass".as_bytes()),
            "client",
            true,
            None,
            Vec::new(),
        );
        packet
            .properties
            .push(ConnectProperty::ReceiveMaximum(20.into()))
            .unwrap();
        packet
    }

    const EXAMPLE_DATA_CLIENTID_USERNAME_PASSWORD: [u8; 36] = [
        0x10, 0x22, 0x00, 0x04, 0x4d, 0x51, 0x54, 0x54, 0x05, 0xC2, 0x00, 0x3c, 0x03, 0x21, 0x00,
        0x14, 0x00, 0x06, 0x63, 0x6c, 0x69, 0x65, 0x6e, 0x74, 0x00, 0x04, 0x75, 0x73, 0x65, 0x72,
        0x00, 0x04, 0x70, 0x61, 0x73, 0x73,
    ];

    fn encode_decode_and_check<const PROPERTIES_N: usize>(
        packet: &Connect<'_, PROPERTIES_N>,
        encoded: &[u8],
    ) {
        encode_and_check(packet, encoded);
        decode_and_check(packet, encoded);
    }

    fn encode_and_check<const PROPERTIES_N: usize>(
        packet: &Connect<'_, PROPERTIES_N>,
        encoded: &[u8],
    ) {
        let mut buf = [0u8; 1024];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf[0..encoded.len()]);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(&buf[0..len], encoded);
    }

    fn decode_and_check<const PROPERTIES_N: usize>(
        packet: &Connect<'_, PROPERTIES_N>,
        encoded: &[u8],
    ) {
        let mut r = MqttBufReader::new(encoded);
        let read_packet: Connect<'_, PROPERTIES_N> = r.get().unwrap();
        assert_eq!(&read_packet, packet);
        assert_eq!(r.position(), encoded.len());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn encode_example() {
        encode_decode_and_check(&example_packet(), &EXAMPLE_DATA);
    }

    #[test]
    fn encode_example_username() {
        encode_decode_and_check(&example_packet_username(), &EXAMPLE_DATA_USERNAME);
    }

    #[test]
    fn encode_example_username_password() {
        encode_decode_and_check(
            &example_packet_username_password(),
            &EXAMPLE_DATA_USERNAME_PASSWORD,
        );
    }

    #[test]
    fn encode_example_clientid_username_password() {
        encode_decode_and_check(
            &example_packet_clientid_username_password(),
            &EXAMPLE_DATA_CLIENTID_USERNAME_PASSWORD,
        );
    }

    #[test]
    fn encode_example_will() {
        encode_decode_and_check(&example_packet_will(), &EXAMPLE_DATA_WILL);
    }

    #[test]
    fn encode_example_will2() {
        encode_decode_and_check(&example_packet_will2(), &EXAMPLE_DATA_WILL2);
    }
}
