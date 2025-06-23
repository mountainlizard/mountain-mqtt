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
pub struct Puback<'a, const P: usize> {
    packet_identifier: PacketIdentifier,
    reason_code: PublishReasonCode,
    properties: Vec<PubackProperty<'a>, P>,
}

impl<'a, const P: usize> Puback<'a, P> {
    pub fn new(
        packet_identifier: PacketIdentifier,
        reason_code: PublishReasonCode,
        properties: Vec<PubackProperty<'a>, P>,
    ) -> Self {
        Self {
            packet_identifier,
            reason_code,
            properties,
        }
    }

    pub fn packet_identifier(&self) -> &PacketIdentifier {
        &self.packet_identifier
    }
    pub fn reason_code(&self) -> &PublishReasonCode {
        &self.reason_code
    }
    pub fn properties(&self) -> &Vec<PubackProperty<'a>, P> {
        &self.properties
    }
}

impl<const P: usize> Packet for Puback<'_, P> {
    fn packet_type(&self) -> PacketType {
        PacketType::Puback
    }
}

impl<const P: usize> PacketWrite for Puback<'_, P> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Always write the packet identifier
        writer.put_u16(self.packet_identifier.0)?;

        if self.properties.is_empty() {
            // Special cases:
            // 1. If there are no properties and reason code is not success, we encode
            //    just the reason code, length will be 3, see "3.4.2.2.1 Property Length"
            // 2. If there are no properties and reason code is success, we don't encode
            //    either. Length will be 2. See end of section "3.4.2.1 PUBACK Reason Code"
            if self.reason_code != PublishReasonCode::Success {
                writer.put(&self.reason_code)?;
            }

        // Normal case, encode everything
        } else {
            writer.put(&self.reason_code)?;
            writer.put_variable_u32_delimited_vec(&self.properties)?; // 3.4.2.2
        }

        // Payload: empty

        Ok(())
    }
}

impl<'a, const P: usize> PacketRead<'a> for Puback<'a, P> {
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
            // See end of section "3.4.2.1 PUBACK Reason Code"
            2 => Ok(Puback::new(
                packet_identifier,
                PublishReasonCode::Success,
                Vec::new(),
            )),
            // See "3.4.2.2.1 Property Length"
            3 => {
                let reason_code = reader.get()?;
                Ok(Puback::new(packet_identifier, reason_code, Vec::new()))
            }
            _ => {
                let reason_code = reader.get()?;
                let mut properties = Vec::new();
                reader.get_property_list(&mut properties)?;
                Ok(Puback::new(packet_identifier, reason_code, properties))
            }
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

    const EXAMPLE_DATA_LENGTH3: [u8; 5] = [0x40, 0x03, 0x8A, 0x5C, 0x87];

    fn example_packet_length3<'a>() -> Puback<'a, 0> {
        let packet_identifier = PacketIdentifier(35420);
        let reason_code = PublishReasonCode::NotAuthorized;

        let packet: Puback<'_, 0> = Puback::new(packet_identifier, reason_code, Vec::new());
        packet
    }

    fn encode_decode_and_check<const P: usize>(packet: &Puback<'_, P>, encoded: &[u8]) {
        encode_and_check(packet, encoded);
        decode_and_check(packet, encoded);
    }

    fn encode_and_check<const P: usize>(packet: &Puback<'_, P>, encoded: &[u8]) {
        let mut buf = [0u8; 1024];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf[0..encoded.len()]);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(&buf[0..len], encoded);
    }

    fn decode_and_check<const P: usize>(packet: &Puback<'_, P>, encoded: &[u8]) {
        let mut r = MqttBufReader::new(encoded);
        let read_packet: Puback<'_, P> = r.get().unwrap();
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
}
