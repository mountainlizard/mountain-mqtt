use super::packet::{Packet, PacketRead, PacketWrite};
use crate::codec::{
    mqtt_reader::{self, MqttReader},
    mqtt_writer::{self, MqttWriter},
};
use crate::data::{
    packet_type::PacketType, property::ConnackProperty, reason_code::ConnectReasonCode,
};
use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Connack<'a, const PROPERTIES_N: usize> {
    session_present: bool,
    reason_code: ConnectReasonCode,
    properties: Vec<ConnackProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize> Connack<'a, PROPERTIES_N> {
    pub fn new(
        session_present: bool,
        reason_code: ConnectReasonCode,
        properties: Vec<ConnackProperty<'a>, PROPERTIES_N>,
    ) -> Self {
        Self {
            session_present,
            reason_code,
            properties,
        }
    }
}

impl<const PROPERTIES_N: usize> Packet for Connack<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Connack
    }
}

impl<const PROPERTIES_N: usize> PacketWrite for Connack<'_, PROPERTIES_N> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:

        // Write the fixed parts of the variable header
        // Note this byte is technically "connack_flags", but only contains one bit of data, which
        // is encoded the same way as a bool-zero-one used for other data
        writer.put_bool_zero_one(self.session_present)?; // 3.2.2.1 Connect Acknowledge Flags
        writer.put_u8(self.reason_code as u8)?; // 3.2.2.2 Connect Reason Code

        // Write the properties vec (3.2.2.3)
        writer.put_variable_u32_delimited_vec(&self.properties)?;

        // Payload: Empty

        Ok(())
    }
}

impl<'a, const PROPERTIES_N: usize> PacketRead<'a> for Connack<'a, PROPERTIES_N> {
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        _len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // Note this byte is technically "connack_flags", but only contains one bit of data, which
        // is encoded the same way as a bool-zero-one used for other data
        let session_present = reader.get_bool_zero_one()?;

        let reason_code = reader.get()?;

        let mut packet: Connack<'a, PROPERTIES_N> =
            Connack::new(session_present, reason_code, Vec::new());

        // Add properties into packet
        reader.get_property_list(&mut packet.properties)?;

        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{
        mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter, read::Read, write::Write,
    };

    use super::*;

    fn example_packet<'a>() -> Connack<'a, 2> {
        let mut properties = Vec::new();
        properties
            .push(ConnackProperty::ReceiveMaximum(21.into()))
            .unwrap();

        let packet: Connack<'_, 2> = Connack::new(true, ConnectReasonCode::ServerMoved, properties);
        packet
    }

    const EXAMPLE_DATA: [u8; 8] = [
        0x20,
        0x06,
        0x01,
        ConnectReasonCode::ServerMoved as u8,
        0x03,
        0x21,
        0x00,
        0x15,
    ];

    #[test]
    fn encode_example() {
        let packet = example_packet();

        let mut buf = [0; EXAMPLE_DATA.len()];
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
        assert_eq!(Connack::read(&mut r).unwrap(), example_packet());
    }
}
