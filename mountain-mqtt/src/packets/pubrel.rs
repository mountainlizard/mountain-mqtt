use super::packet::{Packet, PacketRead, PacketWrite};
use crate::codec::{
    mqtt_reader::{self, MqttReader},
    mqtt_writer::{self, MqttWriter},
};
use crate::data::{
    packet_identifier::PacketIdentifier, packet_type::PacketType, property::PubrelProperty,
    reason_code::PubrelReasonCode,
};

use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Pubrel<'a, const P: usize> {
    packet_identifier: PacketIdentifier,
    reason_code: PubrelReasonCode,
    properties: Vec<PubrelProperty<'a>, P>,
}

impl<'a, const P: usize> Pubrel<'a, P> {
    pub fn new(
        packet_identifier: PacketIdentifier,
        reason_code: PubrelReasonCode,
        properties: Vec<PubrelProperty<'a>, P>,
    ) -> Self {
        Self {
            packet_identifier,
            reason_code,
            properties,
        }
    }
}

impl<const P: usize> Packet for Pubrel<'_, P> {
    fn packet_type(&self) -> PacketType {
        PacketType::Pubrel
    }
}

impl<const P: usize> PacketWrite for Pubrel<'_, P> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Always write the packet identifier
        writer.put_u16(self.packet_identifier.0)?;

        if self.properties.is_empty() {
            // Special cases:
            // 1. If there are no properties and reason code is not success, we encode
            //    just the reason code, length will be 3, see "3.6.2.2.1 Property Length"
            // 2. If there are no properties and reason code is success, we don't encode
            //    either. Length will be 2. See end of section "3.6.2.1 PUBREL Reason Code"
            if self.reason_code != PubrelReasonCode::Success {
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

impl<'a, const P: usize> PacketRead<'a> for Pubrel<'a, P> {
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
            // See end of section "3.6.2.1 PUBREC Reason Code"
            2 => Ok(Pubrel::new(
                packet_identifier,
                PubrelReasonCode::Success,
                Vec::new(),
            )),
            // See "3.6.2.2.1 Property Length"
            3 => {
                let reason_code = reader.get()?;
                Ok(Pubrel::new(packet_identifier, reason_code, Vec::new()))
            }
            _ => {
                let reason_code = reader.get()?;
                let mut properties = Vec::new();
                reader.get_property_list(&mut properties)?;
                Ok(Pubrel::new(packet_identifier, reason_code, properties))
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
        0x62, 0x0C, 0x8A, 0x5C, 0x00, 0x08, 0x1F, 0x00, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f,
    ];

    fn example_packet<'a>() -> Pubrel<'a, 1> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PubrelReasonCode::Success;

        let mut properties = Vec::new();
        properties
            .push(PubrelProperty::ReasonString("Hello".into()))
            .unwrap();
        let packet: Pubrel<'_, 1> = Pubrel::new(packet_identifier, reason_code, properties);
        packet
    }

    const EXAMPLE_DATA_LENGTH2: [u8; 4] = [0x62, 0x02, 0x8A, 0x5C];

    fn example_packet_length2<'a>() -> Pubrel<'a, 0> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PubrelReasonCode::Success;

        let packet: Pubrel<'_, 0> = Pubrel::new(packet_identifier, reason_code, Vec::new());
        packet
    }

    const EXAMPLE_DATA_LENGTH3: [u8; 5] = [0x62, 0x03, 0x8A, 0x5C, 0x92];

    fn example_packet_length3<'a>() -> Pubrel<'a, 0> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PubrelReasonCode::PacketIdentifierNotFound;

        let packet: Pubrel<'_, 0> = Pubrel::new(packet_identifier, reason_code, Vec::new());
        packet
    }

    const EXAMPLE_DATA_USER_PROPERTY: [u8; 21] = [
        0x62, 0x13, 0x30, 0x39, 0x92, 0x0F, 0x26, 0x00, 0x04, 0x68, 0x61, 0x68, 0x61, 0x00, 0x06,
        0x68, 0x65, 0x68, 0x65, 0x38, 0x39,
    ];

    fn example_packet_user_property<'a>() -> Pubrel<'a, 1> {
        let packet_identifier = PacketIdentifier(12345);
        let reason_code = PubrelReasonCode::PacketIdentifierNotFound;

        let pair = StringPair::new("haha", "hehe89");
        let mut properties = Vec::new();
        properties
            .push(PubrelProperty::UserProperty(pair.into()))
            .unwrap();
        let packet: Pubrel<'_, 1> = Pubrel::new(packet_identifier, reason_code, properties);
        packet
    }

    fn encode_decode_and_check<const P: usize>(
        packet: &Pubrel<'_, P>,
        encoded: &[u8],
    ) {
        encode_and_check(packet, encoded);
        decode_and_check(packet, encoded);
    }

    fn encode_and_check<const P: usize>(
        packet: &Pubrel<'_, P>,
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

    fn decode_and_check<const P: usize>(
        packet: &Pubrel<'_, P>,
        encoded: &[u8],
    ) {
        let mut r = MqttBufReader::new(encoded);
        let read_packet: Pubrel<'_, P> = r.get().unwrap();
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
