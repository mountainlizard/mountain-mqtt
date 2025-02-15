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
pub struct Disconnect<'a, const PROPERTIES_N: usize> {
    reason_code: DisconnectReasonCode,
    properties: Vec<DisconnectProperty<'a>, PROPERTIES_N>,
}

impl<const PROPERTIES_N: usize> Disconnect<'_, PROPERTIES_N> {
    pub fn new(reason_code: DisconnectReasonCode) -> Self {
        Self {
            reason_code,
            properties: Vec::new(),
        }
    }
}

impl<const PROPERTIES_N: usize> Default for Disconnect<'_, PROPERTIES_N> {
    fn default() -> Self {
        Self::new(DisconnectReasonCode::Success)
    }
}

impl<const PROPERTIES_N: usize> Packet for Disconnect<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Disconnect
    }
}

impl<const PROPERTIES_N: usize> PacketWrite for Disconnect<'_, PROPERTIES_N> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Special case - if we have reason code success and no properties,
        // output no data
        if self.reason_code == DisconnectReasonCode::Success && self.properties.is_empty() {
            Ok(())
        } else {
            // Variable header:

            // 3.14.2.1 Disconnect Reason Code
            writer.put(&self.reason_code)?;

            // 3.14.2.2 DISCONNECT Properties
            writer.put_variable_u32_delimited_vec(&self.properties)?;

            // Payload: Empty

            Ok(())
        }
    }
}

impl<'a, const PROPERTIES_N: usize> PacketRead<'a> for Disconnect<'a, PROPERTIES_N> {
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // Special case - if length of variable header and payload is 0, treat as
        // reason code success, with no properties
        if len == 0 {
            Ok(Disconnect::new(DisconnectReasonCode::Success))
        } else {
            let reason_code = reader.get()?;
            let mut packet = Disconnect::new(reason_code);
            reader.get_property_list(&mut packet.properties)?;

            Ok(packet)
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
        let mut packet: Disconnect<'_, 1> = Disconnect::new(DisconnectReasonCode::Success);
        packet
            .properties
            .push(DisconnectProperty::SessionExpiryInterval(512.into()))
            .unwrap();
        packet
    }

    const EXAMPLE_DATA: [u8; 9] = [0xE0, 0x07, 0x00, 0x05, 0x11, 0x00, 0x00, 0x02, 0x00];

    fn example_packet_zero_length<'a>() -> Disconnect<'a, 0> {
        Disconnect::new(DisconnectReasonCode::Success)
    }
    const EXAMPLE_DATA_ZERO_LENGTH: [u8; 2] = [0xE0, 0x00];

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
}
