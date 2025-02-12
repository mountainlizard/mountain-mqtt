use super::{
    packet::{Packet, PacketRead, PacketWrite},
    packet_identifier::PacketIdentifier,
    packet_type::PacketType,
    property::SubackProperty,
    reason_code::SubscriptionReasonCode,
};
use crate::data::{
    mqtt_reader::{self, MqttReader, MqttReaderError},
    mqtt_writer::{self, MqttWriter},
};
use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Suback<'a, const PROPERTIES_N: usize, const REQUEST_N: usize> {
    packet_identifier: PacketIdentifier,
    subscription_reason_codes: Vec<SubscriptionReasonCode, REQUEST_N>,
    properties: Vec<SubackProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize, const REQUEST_N: usize> Suback<'a, PROPERTIES_N, REQUEST_N> {
    pub fn new(
        packet_identifier: PacketIdentifier,
        subscription_reason_codes: Vec<SubscriptionReasonCode, REQUEST_N>,
        properties: Vec<SubackProperty<'a>, PROPERTIES_N>,
    ) -> Self {
        Self {
            packet_identifier,
            subscription_reason_codes,
            properties,
        }
    }
}

impl<const PROPERTIES_N: usize, const REQUEST_N: usize> Packet
    for Suback<'_, PROPERTIES_N, REQUEST_N>
{
    fn packet_type(&self) -> PacketType {
        PacketType::Suback
    }
}

impl<const PROPERTIES_N: usize, const REQUEST_N: usize> PacketWrite
    for Suback<'_, PROPERTIES_N, REQUEST_N>
{
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:
        writer.put_u16(self.packet_identifier.0)?; // 3.9.2 SUBACK Variable Header
        writer.put_variable_u32_delimited_vec(&self.properties)?; // 3.9.2.1 SUBACK Properties

        // Payload:
        // Note we just put the reason codes in without a delimiter, they end at the end of the packet
        for r in self.subscription_reason_codes.iter() {
            writer.put(r)?;
        }

        Ok(())
    }
}

impl<'a, const PROPERTIES_N: usize, const REQUEST_N: usize> PacketRead<'a>
    for Suback<'a, PROPERTIES_N, REQUEST_N>
{
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // The payload is a concatenated list of reason codes, we need to
        // know the end position to know where to stop
        let payload_end_position = reader.position() + len;

        // Variable header:
        let packet_identifier = PacketIdentifier(reader.get_u16()?);
        let mut properties = Vec::new();
        reader.get_variable_u32_delimited_vec(&mut properties)?;

        // Payload:
        // Read subscription requests until we run out of data
        let mut subscription_reason_codes = Vec::new();
        while reader.position() < payload_end_position {
            let code = reader.get()?;
            subscription_reason_codes
                .push(code)
                .map_err(|_e| MqttReaderError::MalformedPacket)?;
        }

        let packet = Suback::new(packet_identifier, subscription_reason_codes, properties);
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{
        mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter, read::Read, write::Write,
    };

    use super::*;

    fn example_packet<'a>() -> Suback<'a, 1, 3> {
        let mut subscription_reason_codes = Vec::new();
        subscription_reason_codes
            .push(SubscriptionReasonCode::UnspecifiedError)
            .unwrap();
        subscription_reason_codes
            .push(SubscriptionReasonCode::ImplementationSpecificError)
            .unwrap();
        subscription_reason_codes
            .push(SubscriptionReasonCode::NotAuthorized)
            .unwrap();
        let mut properties = Vec::new();
        properties
            .push(SubackProperty::ReasonString("reasonString".into()))
            .unwrap();
        let packet = Suback::new(
            PacketIdentifier(52232),
            subscription_reason_codes,
            properties,
        );
        packet
    }

    const EXAMPLE_DATA: [u8; 23] = [
        0x90, 0x15, 0xCC, 0x08, 0x0F, 0x1F, 0x00, 0x0C, 0x72, 0x65, 0x61, 0x73, 0x6f, 0x6e, 0x53,
        0x74, 0x72, 0x69, 0x6e, 0x67, 0x80, 0x83, 0x87,
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
        assert_eq!(Suback::read(&mut r).unwrap(), example_packet());
    }
}
