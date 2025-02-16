use super::packet::{Packet, PacketRead, PacketWrite};
use crate::codec::{
    mqtt_reader::{self, MqttReader},
    mqtt_writer::{self, MqttWriter},
};
use crate::data::{
    packet_identifier::PacketIdentifier, packet_type::PacketType, property::PubrecProperty,
    reason_code::PublishReasonCode,
};

use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Pubrec<'a, const PROPERTIES_N: usize> {
    packet_identifier: PacketIdentifier,
    reason_code: PublishReasonCode,
    properties: Vec<PubrecProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize> Pubrec<'a, PROPERTIES_N> {
    pub fn new(
        packet_identifier: PacketIdentifier,
        reason_code: PublishReasonCode,
        properties: Vec<PubrecProperty<'a>, PROPERTIES_N>,
    ) -> Self {
        Self {
            packet_identifier,
            reason_code,
            properties,
        }
    }
}

impl<const PROPERTIES_N: usize> Packet for Pubrec<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Pubrec
    }
}

impl<const PROPERTIES_N: usize> PacketWrite for Pubrec<'_, PROPERTIES_N> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Always write the packet identifier
        writer.put_u16(self.packet_identifier.0)?;

        if self.properties.is_empty() {
            // Special cases:
            // 1. If there are no properties and reason code is not success, we encode
            //    just the reason code, length will be 3, see "3.5.2.2.1 Property Length"
            // 2. If there are no properties and reason code is success, we don't encode
            //    either. Length will be 2. See end of section "3.5.2.1 PUBREC Reason Code"
            if self.reason_code != PublishReasonCode::Success {
                writer.put(&self.reason_code)?;
            }

        // Normal case, encode everything
        } else {
            writer.put(&self.reason_code)?;
            writer.put_variable_u32_delimited_vec(&self.properties)?;
        }

        // Payload: empty

        Ok(())
    }
}

impl<'a, const PROPERTIES_N: usize> PacketRead<'a> for Pubrec<'a, PROPERTIES_N> {
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

        // Special cases for omitted properties, and possbly omitted reason code
        match len {
            // See end of section "3.5.2.1 PUBREC Reason Code"
            2 => Ok(Pubrec::new(
                packet_identifier,
                PublishReasonCode::Success,
                Vec::new(),
            )),
            // See "3.5.2.2.1 Property Length"
            3 => {
                let reason_code = reader.get()?;
                Ok(Pubrec::new(packet_identifier, reason_code, Vec::new()))
            }
            _ => {
                let reason_code = reader.get()?;
                let mut properties = Vec::new();
                reader.get_property_list(&mut properties)?;
                Ok(Pubrec::new(packet_identifier, reason_code, properties))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        codec::{mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter, write::Write},
        data::string_pair::StringPair,
    };

    use super::*;

    const EXAMPLE_DATA: [u8; 14] = [
        0x50, 0x0C, 0x8A, 0x5C, 0x00, 0x08, 0x1F, 0x00, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f,
    ];

    fn example_packet<'a>() -> Pubrec<'a, 1> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PublishReasonCode::Success;

        let mut properties = Vec::new();
        properties
            .push(PubrecProperty::ReasonString("Hello".into()))
            .unwrap();
        let packet: Pubrec<'_, 1> = Pubrec::new(packet_identifier, reason_code, properties);
        packet
    }

    const EXAMPLE_DATA_LENGTH2: [u8; 4] = [0x50, 0x02, 0x8A, 0x5C];

    fn example_packet_length2<'a>() -> Pubrec<'a, 0> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PublishReasonCode::Success;

        let packet: Pubrec<'_, 0> = Pubrec::new(packet_identifier, reason_code, Vec::new());
        packet
    }

    const EXAMPLE_DATA_LENGTH3: [u8; 5] = [0x50, 0x03, 0x8A, 0x5C, 0x83];

    fn example_packet_length3<'a>() -> Pubrec<'a, 0> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PublishReasonCode::ImplementationSpecificError;

        let packet: Pubrec<'_, 0> = Pubrec::new(packet_identifier, reason_code, Vec::new());
        packet
    }

    const EXAMPLE_DATA_USER_PROPERTY: [u8; 20] = [
        0x50, 0x12, 0x8A, 0x5C, 0x90, 0x0E, 0x26, 0x00, 0x05, 0x6e, 0x61, 0x6d, 0x65, 0x31, 0x00,
        0x04, 0x76, 0x61, 0x6c, 0x31,
    ];

    fn example_packet_user_property<'a>() -> Pubrec<'a, 1> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PublishReasonCode::TopicNameInvalid;

        let pair = StringPair::new("name1", "val1");
        let mut properties = Vec::new();
        properties
            .push(PubrecProperty::UserProperty(pair.into()))
            .unwrap();
        let packet: Pubrec<'_, 1> = Pubrec::new(packet_identifier, reason_code, properties);
        packet
    }

    fn encode_decode_and_check<const PROPERTIES_N: usize>(
        packet: &Pubrec<'_, PROPERTIES_N>,
        encoded: &[u8],
    ) {
        encode_and_check(packet, encoded);
        decode_and_check(packet, encoded);
    }

    fn encode_and_check<const PROPERTIES_N: usize>(
        packet: &Pubrec<'_, PROPERTIES_N>,
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
        packet: &Pubrec<'_, PROPERTIES_N>,
        encoded: &[u8],
    ) {
        let mut r = MqttBufReader::new(encoded);
        let read_packet: Pubrec<'_, PROPERTIES_N> = r.get().unwrap();
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

    #[test]
    fn encode_and_decode_example_length3() {
        encode_decode_and_check(&example_packet_length3(), &EXAMPLE_DATA_LENGTH3);
    }

    #[test]
    fn encode_and_decode_example_user_property() {
        encode_decode_and_check(&example_packet_user_property(), &EXAMPLE_DATA_USER_PROPERTY);
    }
}
