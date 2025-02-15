use super::packet::{Packet, PacketRead, PacketWrite};
use crate::codec::{
    mqtt_reader::{self, MqttReader},
    mqtt_writer::{self, MqttWriter},
};
use crate::data::{
    packet_identifier::PacketIdentifier, packet_type::PacketType, property::PubackProperty,
    reason_code::PublishReasonCode,
};
use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Puback<'a, const PROPERTIES_N: usize> {
    packet_identifier: PacketIdentifier,
    reason_code: PublishReasonCode,
    properties: Vec<PubackProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize> Puback<'a, PROPERTIES_N> {
    pub fn new(
        packet_identifier: PacketIdentifier,
        reason_code: PublishReasonCode,
        properties: Vec<PubackProperty<'a>, PROPERTIES_N>,
    ) -> Self {
        Self {
            packet_identifier,
            reason_code,
            properties,
        }
    }
}

impl<const PROPERTIES_N: usize> Packet for Puback<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Puback
    }
}

impl<const PROPERTIES_N: usize> PacketWrite for Puback<'_, PROPERTIES_N> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Always write the packet identifier
        writer.put_u16(self.packet_identifier.0)?;

        // Special case - if the reason code is success and there are no
        // properties, we can just skip the reason code and properties,
        // this will be detected by having only two bytes of length for
        // the packet identifier
        if self.reason_code != PublishReasonCode::Success || !self.properties.is_empty() {
            writer.put(&self.reason_code)?;

            // Write the properties vec (3.4.2.2)
            writer.put_variable_u32_delimited_vec(&self.properties)?;
        }
        // Payload: empty

        Ok(())
    }
}

impl<'a, const PROPERTIES_N: usize> PacketRead<'a> for Puback<'a, PROPERTIES_N> {
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // Always read packet identifier
        let packet_identifier = PacketIdentifier(reader.get_u16()?);

        // If the length is 2, there is no more data and this is the special case
        // where we assume reason code success and no properties, otherwise we
        // read the rest of the fields
        if len == 2 {
            Ok(Puback::new(
                packet_identifier,
                PublishReasonCode::Success,
                Vec::new(),
            ))
        } else {
            let reason_code = reader.get()?;
            let mut properties = Vec::new();
            reader.get_property_list(&mut properties)?;
            Ok(Puback::new(packet_identifier, reason_code, properties))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter, write::Write};

    use super::*;

    const EXAMPLE_DATA: [u8; 14] = [
        0x40, 0x0C, 0x8A, 0x5C, 0x00, 0x08, 0x1F, 0x00, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f,
    ];

    fn example_packet<'a>() -> Puback<'a, 1> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PublishReasonCode::Success;

        let mut properties = Vec::new();
        properties
            .push(PubackProperty::ReasonString("Hello".into()))
            .unwrap();
        let packet: Puback<'_, 1> = Puback::new(packet_identifier, reason_code, properties);
        packet
    }

    const EXAMPLE_DATA_LENGTH2: [u8; 4] = [0x40, 0x02, 0x8A, 0x5C];

    fn example_packet_length2<'a>() -> Puback<'a, 0> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PublishReasonCode::Success;

        let packet: Puback<'_, 0> = Puback::new(packet_identifier, reason_code, Vec::new());
        packet
    }

    fn encode_decode_and_check<const PROPERTIES_N: usize>(
        packet: &Puback<'_, PROPERTIES_N>,
        encoded: &[u8],
    ) {
        encode_and_check(packet, encoded);
        decode_and_check(packet, encoded);
    }

    fn encode_and_check<const PROPERTIES_N: usize>(
        packet: &Puback<'_, PROPERTIES_N>,
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
        packet: &Puback<'_, PROPERTIES_N>,
        encoded: &[u8],
    ) {
        let mut r = MqttBufReader::new(encoded);
        let read_packet: Puback<'_, PROPERTIES_N> = r.get().unwrap();
        assert_eq!(&read_packet, packet);
        assert_eq!(r.position(), encoded.len());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn encode_and_decode_example() {
        encode_decode_and_check(&example_packet(), &EXAMPLE_DATA);
    }

    #[test]
    fn encode_and_decode_example_length2() {
        encode_decode_and_check(&example_packet_length2(), &EXAMPLE_DATA_LENGTH2);
    }
}
