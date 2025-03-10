use super::packet::{Packet, PacketRead, PacketWrite};
use crate::codec::{
    mqtt_reader::{self, MqttReader},
    mqtt_writer::{self, MqttWriter},
};
use crate::data::{
    packet_identifier::PacketIdentifier, packet_type::PacketType, property::PubcompProperty,
    reason_code::PubrelReasonCode,
};
use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Pubcomp<'a, const P: usize> {
    packet_identifier: PacketIdentifier,
    // Note that the pubcomp reason codes match the pubrel ones, so we
    // reuse them, to match general approach of naming reason codes after
    // the message they reply to (e.g. puback uses publish reason codes)
    reason_code: PubrelReasonCode,
    properties: Vec<PubcompProperty<'a>, P>,
}

impl<'a, const P: usize> Pubcomp<'a, P> {
    pub fn new(
        packet_identifier: PacketIdentifier,
        reason_code: PubrelReasonCode,
        properties: Vec<PubcompProperty<'a>, P>,
    ) -> Self {
        Self {
            packet_identifier,
            reason_code,
            properties,
        }
    }
}

impl<const P: usize> Packet for Pubcomp<'_, P> {
    fn packet_type(&self) -> PacketType {
        PacketType::Pubcomp
    }
}

impl<const P: usize> PacketWrite for Pubcomp<'_, P> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Always write the packet identifier
        writer.put_u16(self.packet_identifier.0)?;

        if self.properties.is_empty() {
            // Special cases:
            // 1. If there are no properties and reason code is not success, we encode
            //    just the reason code, length will be 3, see "3.7.2.2.1 Property Length"
            // 2. If there are no properties and reason code is success, we don't encode
            //    either. Length will be 2. See end of section "3.7.2.1 PUBCOMP Reason Code"
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

impl<'a, const P: usize> PacketRead<'a> for Pubcomp<'a, P> {
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
            // See end of section "3.7.2.1 PUBCOMP Reason Code"
            2 => Ok(Pubcomp::new(
                packet_identifier,
                PubrelReasonCode::Success,
                Vec::new(),
            )),
            // See "3.7.2.2.1 Property Length"
            3 => {
                let reason_code = reader.get()?;
                Ok(Pubcomp::new(packet_identifier, reason_code, Vec::new()))
            }
            _ => {
                let reason_code = reader.get()?;
                let mut properties = Vec::new();
                reader.get_property_list(&mut properties)?;
                Ok(Pubcomp::new(packet_identifier, reason_code, properties))
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
        0x70, 0x0C, 0x8A, 0x5C, 0x00, 0x08, 0x1F, 0x00, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f,
    ];

    fn example_packet<'a>() -> Pubcomp<'a, 1> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PubrelReasonCode::Success;

        let mut properties = Vec::new();
        properties
            .push(PubcompProperty::ReasonString("Hello".into()))
            .unwrap();
        let packet: Pubcomp<'_, 1> = Pubcomp::new(packet_identifier, reason_code, properties);
        packet
    }

    const EXAMPLE_DATA_LENGTH2: [u8; 4] = [0x70, 0x02, 0x8A, 0x5C];

    fn example_packet_length2<'a>() -> Pubcomp<'a, 0> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PubrelReasonCode::Success;

        let packet: Pubcomp<'_, 0> = Pubcomp::new(packet_identifier, reason_code, Vec::new());
        packet
    }

    const EXAMPLE_DATA_LENGTH3: [u8; 5] = [0x70, 0x03, 0x8A, 0x5C, 0x92];

    fn example_packet_length3<'a>() -> Pubcomp<'a, 0> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PubrelReasonCode::PacketIdentifierNotFound;

        let packet: Pubcomp<'_, 0> = Pubcomp::new(packet_identifier, reason_code, Vec::new());
        packet
    }

    const EXAMPLE_DATA_USER_PROPERTY: [u8; 21] = [
        0x70, 0x13, 0x30, 0x39, 0x92, 0x0F, 0x26, 0x00, 0x04, 0x68, 0x61, 0x68, 0x61, 0x00, 0x06,
        0x68, 0x65, 0x68, 0x65, 0x38, 0x39,
    ];

    fn example_packet_user_property<'a>() -> Pubcomp<'a, 1> {
        let packet_identifier = PacketIdentifier(12345);
        let reason_code = PubrelReasonCode::PacketIdentifierNotFound;

        let pair = StringPair::new("haha", "hehe89");
        let mut properties = Vec::new();
        properties
            .push(PubcompProperty::UserProperty(pair.into()))
            .unwrap();
        let packet: Pubcomp<'_, 1> = Pubcomp::new(packet_identifier, reason_code, properties);
        packet
    }

    const EXAMPLE_DATA_REASON_STRING: [u8; 14] = [
        0x70, 0x0C, 0x8A, 0x5C, 0x00, 0x08, 0x1F, 0x00, 0x05, 0x57, 0x68, 0x65, 0x65, 0x6c,
    ];

    fn example_packet_reason_string<'a>() -> Pubcomp<'a, 1> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PubrelReasonCode::Success;

        let mut properties = Vec::new();
        properties
            .push(PubcompProperty::ReasonString("Wheel".into()))
            .unwrap();
        let packet: Pubcomp<'_, 1> = Pubcomp::new(packet_identifier, reason_code, properties);
        packet
    }

    fn encode_decode_and_check<const P: usize>(packet: &Pubcomp<'_, P>, encoded: &[u8]) {
        encode_and_check(packet, encoded);
        decode_and_check(packet, encoded);
    }

    fn encode_and_check<const P: usize>(packet: &Pubcomp<'_, P>, encoded: &[u8]) {
        let mut buf = [0u8; 1024];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf[0..encoded.len()]);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(&buf[0..len], encoded);
    }

    fn decode_and_check<const P: usize>(packet: &Pubcomp<'_, P>, encoded: &[u8]) {
        let mut r = MqttBufReader::new(encoded);
        let read_packet: Pubcomp<'_, P> = r.get().unwrap();
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

    #[test]
    fn encode_and_decode_example_reason_string() {
        encode_decode_and_check(&example_packet_reason_string(), &EXAMPLE_DATA_REASON_STRING);
    }
}
