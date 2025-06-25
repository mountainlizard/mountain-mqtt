use super::packet::{Packet, PacketRead, PacketWrite};
use crate::data::{
    packet_identifier::PacketIdentifier, packet_type::PacketType, property::SubackProperty,
    reason_code::SubscribeReasonCode,
};
use crate::{
    codec::{
        mqtt_reader::{self, MqttReader},
        mqtt_writer::{self, MqttWriter},
    },
    error::PacketReadError,
};
use heapless::Vec;

#[derive(Debug, PartialEq)]
pub struct Suback<'a, const P: usize, const S: usize> {
    packet_identifier: PacketIdentifier,
    first_reason_code: SubscribeReasonCode,
    other_reason_codes: Vec<SubscribeReasonCode, S>,
    properties: Vec<SubackProperty<'a>, P>,
}

impl<'a, const P: usize, const S: usize> Suback<'a, P, S> {
    pub fn new(
        packet_identifier: PacketIdentifier,
        first_reason_code: SubscribeReasonCode,
        other_reason_codes: Vec<SubscribeReasonCode, S>,
        properties: Vec<SubackProperty<'a>, P>,
    ) -> Self {
        Self {
            packet_identifier,
            first_reason_code,
            other_reason_codes,
            properties,
        }
    }

    pub fn packet_identifier(&self) -> &PacketIdentifier {
        &self.packet_identifier
    }
    pub fn first_reason_code(&self) -> &SubscribeReasonCode {
        &self.first_reason_code
    }
    pub fn other_reason_codes(&self) -> &Vec<SubscribeReasonCode, S> {
        &self.other_reason_codes
    }
    pub fn properties(&self) -> &Vec<SubackProperty<'a>, P> {
        &self.properties
    }
}

impl<const P: usize, const S: usize> Packet for Suback<'_, P, S> {
    fn packet_type(&self) -> PacketType {
        PacketType::Suback
    }
}

impl<const P: usize, const S: usize> PacketWrite for Suback<'_, P, S> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:
        writer.put_u16(self.packet_identifier.0)?; // 3.9.2 SUBACK Variable Header
        writer.put_variable_u32_delimited_vec(&self.properties)?; // 3.9.2.1 SUBACK Properties

        // Payload:
        // Note we just put the reason codes in without a delimiter, they end at the end of the packet
        writer.put(&self.first_reason_code)?;
        for r in self.other_reason_codes.iter() {
            writer.put(r)?;
        }

        Ok(())
    }
}

impl<'a, const P: usize, const S: usize> PacketRead<'a> for Suback<'a, P, S> {
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
        reader.get_property_list(&mut properties)?;

        // Payload:

        // We must have at least one reason code, since any valid subscribe packet we
        // are replying to must have had at least one subscription request [MQTT-3.8.3-2]
        let first_reason_code = reader
            .get()
            .map_err(|_| PacketReadError::SubackWithoutValidReasonCode)?;

        // Read subscription requests until we run out of data
        let mut other_reason_codes = Vec::new();
        while reader.position() < payload_end_position {
            let code = reader.get()?;
            other_reason_codes
                .push(code)
                .map_err(|_e| PacketReadError::TooManyRequests)?;
        }

        let packet = Suback::new(
            packet_identifier,
            first_reason_code,
            other_reason_codes,
            properties,
        );
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{
        mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter, read::Read, write::Write,
    };

    use super::*;

    fn example_packet<'a>() -> Suback<'a, 1, 2> {
        let first_reason_code = SubscribeReasonCode::UnspecifiedError;

        let mut other_reason_codes = Vec::new();
        other_reason_codes
            .push(SubscribeReasonCode::ImplementationSpecificError)
            .unwrap();
        other_reason_codes
            .push(SubscribeReasonCode::NotAuthorized)
            .unwrap();

        let mut properties = Vec::new();
        properties
            .push(SubackProperty::ReasonString("reasonString".into()))
            .unwrap();

        let packet = Suback::new(
            PacketIdentifier(52232),
            first_reason_code,
            other_reason_codes,
            properties,
        );
        packet
    }

    const EXAMPLE_DATA: [u8; 23] = [
        0x90, 0x15, 0xCC, 0x08, // packet id
        0x0F, // properties length
        0x1F, 0x00, 0x0C, 0x72, 0x65, 0x61, 0x73, 0x6f, 0x6e, 0x53, 0x74, 0x72, 0x69, 0x6e,
        0x67, // properties
        0x80, 0x83, 0x87, // reason codes
    ];

    // EXAMPLE_DATA but with no reason codes at all, to check we get SubackWithoutValidReasonCode
    const EXAMPLE_DATA_NO_REASON_CODES: [u8; 20] = [
        0x90, 0x12, 0xCC, 0x08, // packet id
        0x0F, // properties length
        0x1F, 0x00, 0x0C, 0x72, 0x65, 0x61, 0x73, 0x6f, 0x6e, 0x53, 0x74, 0x72, 0x69, 0x6e,
        0x67, // properties
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

    #[test]
    fn decode_should_error_on_no_reason_codes() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA_NO_REASON_CODES);
        let result: Result<Suback<'_, 16, 16>, PacketReadError> = Suback::read(&mut r);
        assert_eq!(result, Err(PacketReadError::SubackWithoutValidReasonCode));
    }
}
