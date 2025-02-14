use super::packet::{Packet, PacketRead, PacketWrite};
use crate::codec::{
    mqtt_reader::{self, MqttReader, MqttReaderError},
    mqtt_writer::{self, MqttWriter},
};
use crate::data::{
    packet_identifier::{PacketIdentifier, PublishPacketIdentifier},
    packet_type::PacketType,
    property::PublishProperty,
};
use heapless::Vec;

const RETAIN_SHIFT: i32 = 0;
const QOS_SHIFT: i32 = 1;
const QOS_MASK: u8 = 0x03;
const DUPLICATE_SHIFT: i32 = 3;

pub fn is_valid_publish_first_header_byte(encoded: u8) -> bool {
    let first_nibble_ok = (encoded & 0xF0) == u8::from(PacketType::Publish);
    let qos_ok = (encoded >> QOS_SHIFT & QOS_MASK) != 3;
    first_nibble_ok && qos_ok
}

#[derive(Debug, PartialEq)]
pub struct Publish<'a, const PROPERTIES_N: usize> {
    duplicate: bool,
    retain: bool,
    topic_name: &'a str,
    publish_packet_identifier: PublishPacketIdentifier,
    payload: &'a [u8],
    properties: Vec<PublishProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize> Publish<'a, PROPERTIES_N> {
    pub fn new(
        duplicate: bool,
        retain: bool,
        topic_name: &'a str,
        packet_identifier: PublishPacketIdentifier,
        payload: &'a [u8],
        properties: Vec<PublishProperty<'a>, PROPERTIES_N>,
    ) -> Self {
        Self {
            duplicate,
            retain,
            topic_name,
            publish_packet_identifier: packet_identifier,
            payload,
            properties,
        }
    }
}

impl<const PROPERTIES_N: usize> Packet for Publish<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Publish
    }

    fn fixed_header_first_byte(&self) -> u8 {
        let mut b: u8 = self.packet_type().into();
        if self.retain {
            b |= 1 << RETAIN_SHIFT;
        }
        b |= (self.publish_packet_identifier.qos() as u8) << QOS_SHIFT;
        if self.duplicate {
            b |= 1 << DUPLICATE_SHIFT;
        }
        b
    }
}

impl<const PROPERTIES_N: usize> PacketWrite for Publish<'_, PROPERTIES_N> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:

        // Write the fixed parts of the variable header
        writer.put_str(self.topic_name)?;
        match &self.publish_packet_identifier {
            PublishPacketIdentifier::None => {} // QoS0, no packet identifier
            PublishPacketIdentifier::Qos1(id) => writer.put_u16(id.0)?,
            PublishPacketIdentifier::Qos2(id) => writer.put_u16(id.0)?,
        }

        // Write the properties vec (3.3.2.3)
        writer.put_variable_u32_delimited_vec(&self.properties)?;

        // Payload - note that we put a raw slice rather than use `put_binary_data`, since
        // the payload has no delimiting length, the length is what's left of the packet
        // after the variable header
        writer.put_slice(self.payload)?;

        Ok(())
    }
}

impl<'a, const PROPERTIES_N: usize> PacketRead<'a> for Publish<'a, PROPERTIES_N> {
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // reader may not start at position 0, so record where we expect the
        // payload to end
        let payload_end_position = reader.position() + len;

        // Data from the first header byte
        let retain = first_header_byte & (1 << RETAIN_SHIFT) != 0;
        let duplicate = first_header_byte & (1 << DUPLICATE_SHIFT) != 0;
        let qos_value = (first_header_byte >> QOS_SHIFT) & QOS_MASK;

        let topic_name = reader.get_str()?;
        let packet_identifier = match qos_value {
            0 => PublishPacketIdentifier::None, // QoS0, no packet identifier
            1 => PublishPacketIdentifier::Qos1(PacketIdentifier(reader.get_u16()?)),
            2 => PublishPacketIdentifier::Qos1(PacketIdentifier(reader.get_u16()?)),
            _ => return Err(mqtt_reader::MqttReaderError::InvalidQoSValue),
        };

        let mut properties = Vec::new();
        reader.get_variable_u32_delimited_vec(&mut properties)?;

        // We expect there to be 0 or more bytes left in data,
        // if so this is all the payload, if not we have a malformed packet
        let position = reader.position();
        if position > payload_end_position {
            Err(MqttReaderError::InsufficientData)
        } else {
            let payload_len = payload_end_position - position;
            let payload = reader.get_slice(payload_len)?;

            let packet: Publish<'a, PROPERTIES_N> = Publish::new(
                duplicate,
                retain,
                topic_name,
                packet_identifier,
                payload,
                properties,
            );

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

    const EXAMPLE_PAYLOAD: [u8; 11] = [
        0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64,
    ];

    const EXAMPLE_LEN: usize = 29;

    const EXAMPLE_DATA: [u8; EXAMPLE_LEN] = [
        0x32, 0x1B, 0x00, 0x04, 0x74, 0x65, 0x73, 0x74, 0x5B, 0x88, 0x07, 0x01, 0x01, 0x02, 0x00,
        0x00, 0xB2, 0x6E, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64,
    ];

    const EXAMPLE_DATA_RETAIN: [u8; EXAMPLE_LEN] = [
        0x33, 0x1B, 0x00, 0x04, 0x74, 0x65, 0x73, 0x74, 0x5B, 0x88, 0x07, 0x01, 0x01, 0x02, 0x00,
        0x00, 0xB2, 0x6E, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64,
    ];

    const EXAMPLE_DATA_DUPLICATE: [u8; EXAMPLE_LEN] = [
        0x3A, 0x1B, 0x00, 0x04, 0x74, 0x65, 0x73, 0x74, 0x5B, 0x88, 0x07, 0x01, 0x01, 0x02, 0x00,
        0x00, 0xB2, 0x6E, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64,
    ];

    const EXAMPLE_DATA_RETAIN_DUPLICATE: [u8; EXAMPLE_LEN] = [
        0x3B, 0x1B, 0x00, 0x04, 0x74, 0x65, 0x73, 0x74, 0x5B, 0x88, 0x07, 0x01, 0x01, 0x02, 0x00,
        0x00, 0xB2, 0x6E, 0x48, 0x65, 0x6c, 0x6c, 0x6f, 0x20, 0x77, 0x6f, 0x72, 0x6c, 0x64,
    ];

    fn example_packet<'a>(duplicate: bool, retain: bool) -> Publish<'a, 2> {
        let packet_identifier = PublishPacketIdentifier::Qos1(PacketIdentifier(23432));
        let mut properties = Vec::new();
        properties
            .push(PublishProperty::PayloadFormatIndicator(0x01.into()))
            .unwrap();
        properties
            .push(PublishProperty::MessageExpiryInterval(45678.into()))
            .unwrap();
        let packet: Publish<'_, 2> = Publish::new(
            duplicate,
            retain,
            "test",
            packet_identifier,
            &EXAMPLE_PAYLOAD,
            properties,
        );
        packet
    }

    fn encode_example(duplicate: bool, retain: bool, example_data: &[u8]) {
        let packet = example_packet(duplicate, retain);

        let mut buf = [0; EXAMPLE_LEN];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(&buf[0..len], example_data);
    }

    #[test]
    fn encode_examples() {
        encode_example(false, false, &EXAMPLE_DATA);
        encode_example(false, true, &EXAMPLE_DATA_RETAIN);
        encode_example(true, false, &EXAMPLE_DATA_DUPLICATE);
        encode_example(true, true, &EXAMPLE_DATA_RETAIN_DUPLICATE);
    }

    #[test]
    fn decode_example() {
        let mut r = MqttBufReader::new(&EXAMPLE_DATA);
        assert_eq!(Publish::read(&mut r).unwrap(), example_packet(false, false));

        let mut r = MqttBufReader::new(&EXAMPLE_DATA_RETAIN);
        assert_eq!(Publish::read(&mut r).unwrap(), example_packet(false, true));

        let mut r = MqttBufReader::new(&EXAMPLE_DATA_DUPLICATE);
        assert_eq!(Publish::read(&mut r).unwrap(), example_packet(true, false));

        let mut r = MqttBufReader::new(&EXAMPLE_DATA_RETAIN_DUPLICATE);
        assert_eq!(Publish::read(&mut r).unwrap(), example_packet(true, true));
    }
}
