use super::{
    packet::{Packet, PacketWrite, PROTOCOL_NAME, PROTOCOL_VERSION_5},
    packet_type::PacketType,
    property::{ConnectProperty, WillProperty},
    quality_of_service::QualityOfService,
};
use crate::data::mqtt_writer::{self, MqttWriter};
use heapless::Vec;

pub struct Will<'a, const PROPERTIES_N: usize> {
    qos: QualityOfService,
    retain: bool,
    topic_name: &'a str,
    payload: &'a [u8],
    properties: Vec<WillProperty<'a>, PROPERTIES_N>,
}

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
    ) -> Self {
        Self {
            keep_alive,
            username,
            password,
            client_id,
            clean_start,
            will,
            properties: Vec::new(),
        }
    }

    fn connect_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.clean_start {
            flags |= 1 << 1;
        }
        if let Some(ref will) = self.will {
            flags |= 1 << 2; // Will present
            flags |= (will.qos as u8) << 3;
            if will.retain {
                flags |= 1 << 5;
            }
        }
        if self.password.is_some() {
            flags |= 1 << 6;
        }
        if self.username.is_some() {
            flags |= 1 << 7;
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

#[cfg(test)]
mod tests {
    use crate::data::{mqtt_writer::MqttBufWriter, write::Write};

    use super::*;

    fn example_packet<'a>() -> Connect<'a, 1> {
        let mut packet = Connect::new(60, None, None, "", true, None);
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

    fn example_packet_username<'a>() -> Connect<'a, 1> {
        let mut packet = Connect::new(60, Some("user"), None, "", true, None);
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
        let mut packet = Connect::new(60, Some("user"), Some("pass".as_bytes()), "", true, None);
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

    #[test]
    fn encode_example() {
        encode_and_check(&example_packet(), &EXAMPLE_DATA);
    }

    #[test]
    fn encode_example_username() {
        encode_and_check(&example_packet_username(), &EXAMPLE_DATA_USERNAME);
    }

    #[test]
    fn encode_example_username_password() {
        encode_and_check(
            &example_packet_username_password(),
            &EXAMPLE_DATA_USERNAME_PASSWORD,
        );
    }

    #[test]
    fn encode_example_clientid_username_password() {
        encode_and_check(
            &example_packet_clientid_username_password(),
            &EXAMPLE_DATA_CLIENTID_USERNAME_PASSWORD,
        );
    }
}
