use super::packet::{Packet, PacketRead, PacketWrite};
use crate::codec::{
    mqtt_reader::{self, MqttReader},
    mqtt_writer::{self, MqttWriter},
};
use crate::data::{
    packet_type::PacketType, property::DisconnectProperty, reason_code::DisconnectReasonCode,
};
use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Disconnect<'a, const P: usize> {
    reason_code: DisconnectReasonCode,
    properties: Vec<DisconnectProperty<'a>, P>,
}

impl<'a, const P: usize> Disconnect<'a, P> {
    pub fn new(
        reason_code: DisconnectReasonCode,
        properties: Vec<DisconnectProperty<'a>, P>,
    ) -> Self {
        Self {
            reason_code,
            properties,
        }
    }

    pub fn reason_code(&self) -> &DisconnectReasonCode {
        &self.reason_code
    }
    pub fn properties(&self) -> &Vec<DisconnectProperty<'a>, P> {
        &self.properties
    }
}

impl Default for Disconnect<'_, 0> {
    fn default() -> Self {
        Self::new(DisconnectReasonCode::Success, Vec::new())
    }
}

impl<const P: usize> Packet for Disconnect<'_, P> {
    fn packet_type(&self) -> PacketType {
        PacketType::Disconnect
    }
}

impl<const P: usize> PacketWrite for Disconnect<'_, P> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:

        // Can encode with 0 length
        if self.reason_code == DisconnectReasonCode::Success && self.properties.is_empty() {
            Ok(())

        // Can encode just reason with 1 length
        } else if self.properties.is_empty() {
            // 3.14.2.1 Disconnect Reason Code
            writer.put(&self.reason_code)?;
            // Skip properties completely
            Ok(())

        // Normal encoding
        } else {
            // 3.14.2.1 Disconnect Reason Code
            writer.put(&self.reason_code)?;

            // 3.14.2.2 DISCONNECT Properties
            writer.put_variable_u32_delimited_vec(&self.properties)?;

            // Payload: Empty

            Ok(())
        }
    }
}

impl<'a, const P: usize> PacketRead<'a> for Disconnect<'a, P> {
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        match len {
            // 0 length, success with no properties
            0 => Ok(Disconnect::new(DisconnectReasonCode::Success, Vec::new())),

            // 1 length, reason code and no properties
            1 => {
                let reason_code = reader.get()?;
                Ok(Disconnect::new(reason_code, Vec::new()))
            }
            // Normal packet
            _ => {
                let reason_code = reader.get()?;

                let mut properties = Vec::new();
                reader.get_property_list(&mut properties)?;

                let packet = Disconnect::new(reason_code, properties);
                Ok(packet)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{
        mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter, read::Read, write::Write,
    };

    use super::*;

    fn example_packet<'a>() -> Disconnect<'a, 1> {
        let mut packet: Disconnect<'_, 1> =
            Disconnect::new(DisconnectReasonCode::Success, Vec::new());
        packet
            .properties
            .push(DisconnectProperty::SessionExpiryInterval(512.into()))
            .unwrap();
        packet
    }

    const EXAMPLE_DATA: [u8; 9] = [0xE0, 0x07, 0x00, 0x05, 0x11, 0x00, 0x00, 0x02, 0x00];

    fn example_packet_zero_length<'a>() -> Disconnect<'a, 0> {
        Disconnect::new(DisconnectReasonCode::Success, Vec::new())
    }
    const EXAMPLE_DATA_ZERO_LENGTH: [u8; 2] = [0xE0, 0x00];

    fn example_packet_one_length<'a>() -> Disconnect<'a, 0> {
        Disconnect::new(DisconnectReasonCode::SessionTakenOver, Vec::new())
    }
    const EXAMPLE_DATA_ONE_LENGTH: [u8; 3] = [0xE0, 0x01, 0x8E];

    #[test]
    fn encode_example() {
        let packet = example_packet();

        let mut buf = [0; 9];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(buf[0..len], EXAMPLE_DATA);
    }

    #[test]
    fn decode_example() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA);
        assert_eq!(Disconnect::read(&mut r).unwrap(), example_packet());
    }

    #[test]
    fn encode_example_zero_length() {
        let packet = example_packet_zero_length();

        let mut buf = [0; 2];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(buf[0..len], EXAMPLE_DATA_ZERO_LENGTH);
    }

    #[test]
    fn decode_example_zero_length() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA_ZERO_LENGTH);
        assert_eq!(
            Disconnect::read(&mut r).unwrap(),
            example_packet_zero_length()
        );
    }

    #[test]
    fn encode_example_one_length() {
        let packet = example_packet_one_length();

        let mut buf = [0; 3];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(buf[0..len], EXAMPLE_DATA_ONE_LENGTH);
    }

    #[test]
    fn decode_example_one_length() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA_ONE_LENGTH);
        assert_eq!(
            Disconnect::read(&mut r).unwrap(),
            example_packet_one_length()
        );
    }
}
