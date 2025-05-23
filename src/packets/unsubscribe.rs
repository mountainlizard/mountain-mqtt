use super::packet::{Packet, PacketRead, PacketWrite};
use crate::data::{
    packet_identifier::PacketIdentifier, packet_type::PacketType, property::UnsubscribeProperty,
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
pub struct Unsubscribe<'a, const P: usize, const S: usize> {
    packet_identifier: PacketIdentifier,
    first_request: &'a str,
    other_requests: Vec<&'a str, S>,
    properties: Vec<UnsubscribeProperty<'a>, P>,
}

impl<'a, const P: usize, const S: usize>
    Unsubscribe<'a, P, S>
{
    pub fn new(
        packet_identifier: PacketIdentifier,
        first_request: &'a str,
        other_requests: Vec<&'a str, S>,
        properties: Vec<UnsubscribeProperty<'a>, P>,
    ) -> Self {
        Self {
            packet_identifier,
            first_request,
            other_requests,
            properties,
        }
    }
}

impl<const P: usize, const S: usize> Packet
    for Unsubscribe<'_, P, S>
{
    fn packet_type(&self) -> PacketType {
        PacketType::Unsubscribe
    }
}

impl<const P: usize, const S: usize> PacketWrite
    for Unsubscribe<'_, P, S>
{
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:
        writer.put_u16(self.packet_identifier.0)?; // 3.10.2 UNSUBSCRIBE Variable Header
        writer.put_variable_u32_delimited_vec(&self.properties)?; // 3.10.2.1 UNSUBSCRIBE Properties

        // Payload:
        writer.put_str(self.first_request)?;
        // Note we just put the requests in without a delimiter, they end at the end of the packet
        for r in self.other_requests.iter() {
            writer.put_str(r)?;
        }

        Ok(())
    }
}

impl<'a, const P: usize, const S: usize> PacketRead<'a>
    for Unsubscribe<'a, P, S>
{
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // The payload is a concatenated list of unsubscription requests, we need to
        // know the end position to know where to stop
        let payload_end_position = reader.position() + len;

        // Variable header:
        let packet_identifier = PacketIdentifier(reader.get_u16()?);
        let mut properties = Vec::new();
        reader.get_property_list(&mut properties)?;

        // Payload:

        // We must have at least one subscription request, otherwise this is a protocol
        // error  [MQTT-3.10.3-2]
        let first_request = reader
            .get_str()
            .map_err(|_| PacketReadError::UnsubscribeWithoutValidSubscriptionRequest)?;

        // Read subscription requests until we run out of data
        let mut other_requests = Vec::new();
        while reader.position() < payload_end_position {
            let other_request = reader.get_str()?;
            other_requests
                .push(other_request)
                .map_err(|_e| PacketReadError::TooManyRequests)?;
        }

        let packet = Unsubscribe::new(packet_identifier, first_request, other_requests, properties);
        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        codec::{mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter, read::Read, write::Write},
        data::string_pair::StringPair,
    };

    use super::*;

    fn example_packet<'a>() -> Unsubscribe<'a, 1, 2> {
        let first_request = "test/topic";
        let mut other_requests = Vec::new();
        other_requests.push("hehe/#").unwrap();

        let mut properties = Vec::new();
        let pair = StringPair::new("haha", "hehe89");
        properties
            .push(UnsubscribeProperty::UserProperty(pair.into()))
            .unwrap();

        let packet = Unsubscribe::new(
            PacketIdentifier(5432),
            first_request,
            other_requests,
            properties,
        );
        packet
    }

    #[rustfmt::skip]
    const EXAMPLE_DATA: [u8; 40] = [
        0xA2, 0x26, 0x15, 0x38, 0x0F, 0x26, 0x00, 0x04, 0x68, 0x61, 0x68, 0x61, 0x00, 0x06, 0x68,
        0x65, 0x68, 0x65, 0x38, 0x39, 
        // test/topic
        0x00, 0x0A, 0x74, 0x65, 0x73, 0x74, 0x2F, 0x74, 0x6F, 0x70, 0x69, 0x63, 
        // hehe/#
        0x00, 0x06, 0x68, 0x65, 0x68, 0x65, 0x2F, 0x23,
    ];

    // As for EXAMPLE_DATA but we only have one request to unsubscribe, and we miss out the last byte
    // to trigger UnsubscribeWithoutValidSubscriptionRequest, check we get this not just InsufficientData
    #[rustfmt::skip]
    const EXAMPLE_DATA_TRUNCATED_REQUEST: [u8; 31] = [
        0xA2, 0x1D, 0x15, 0x38, 0x0F, 0x26, 0x00, 0x04, 0x68, 0x61, 0x68, 0x61, 0x00, 0x06, 0x68,
        0x65, 0x68, 0x65, 0x38, 0x39, 
        // test/topic
        0x00, 0x0A, 0x74, 0x65, 0x73, 0x74, 0x2F, 0x74, 0x6F, 0x70, 0x69,
    ];

    // As for EXAMPLE_DATA but no requests at all
    #[rustfmt::skip]
    const EXAMPLE_DATA_NO_REQUEST: [u8; 20] = [
        0xA2, 0x12, 0x15, 0x38, 0x0F, 0x26, 0x00, 0x04, 0x68, 0x61, 0x68, 0x61, 0x00, 0x06, 0x68,
        0x65, 0x68, 0x65, 0x38, 0x39, 
        // no requests
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
        assert_eq!(Unsubscribe::read(&mut r).unwrap(), example_packet());
    }

    #[test]
    fn decode_should_fail_on_truncated_request() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA_TRUNCATED_REQUEST);
        let result: Result<Unsubscribe<'_, 16, 16>, PacketReadError> = Unsubscribe::read(&mut r);
        assert_eq!(
            result,
            Err(PacketReadError::UnsubscribeWithoutValidSubscriptionRequest)
        );
    }

    #[test]
    fn decode_should_fail_on_no_request() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA_NO_REQUEST);
        let result: Result<Unsubscribe<'_, 16, 16>, PacketReadError> = Unsubscribe::read(&mut r);
        assert_eq!(
            result,
            Err(PacketReadError::UnsubscribeWithoutValidSubscriptionRequest)
        );
    }
}
