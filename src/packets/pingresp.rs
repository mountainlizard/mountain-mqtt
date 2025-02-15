use super::packet::{Packet, PacketRead, PacketWrite};
use crate::codec::{
    mqtt_reader::{self, MqttReader, PacketReadError},
    mqtt_writer::{self, MqttWriter},
};
use crate::data::packet_type::PacketType;

#[derive(Debug, PartialEq)]
pub struct Pingresp {}

impl Pingresp {
    pub fn new() -> Pingresp {
        Pingresp {}
    }
}

impl Default for Pingresp {
    fn default() -> Self {
        Self::new()
    }
}

impl Packet for Pingresp {
    fn packet_type(&self) -> PacketType {
        PacketType::Pingresp
    }
}

impl PacketWrite for Pingresp {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        _writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Empty
        Ok(())
    }
}

impl<'a> PacketRead<'a> for Pingresp {
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        _reader: &mut R,
        _first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // No data
        if len == 0 {
            Ok(Pingresp::default())
        } else {
            Err(PacketReadError::MalformedPacket)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::codec::{
        mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter, read::Read, write::Write,
    };

    use super::*;

    const ENCODED: [u8; 2] = [0xD0, 0x00];
    const ENCODED_NONZERO_LENGTH: [u8; 2] = [0xD0, 0x01];
    const ENCODED_INCORRECT_PACKET_TYPE: [u8; 2] = [0x00, 0x00];

    #[test]
    fn encode() {
        let packet = Pingresp::default();

        let mut buf = [0; ENCODED.len()];
        let len = {
            let mut r = MqttBufWriter::new(&mut buf);
            packet.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(buf[0..len], ENCODED);
    }

    #[test]
    fn decode() {
        let mut r = MqttBufReader::new(&ENCODED);
        assert_eq!(Pingresp::read(&mut r).unwrap(), Pingresp::default());
    }

    #[test]
    fn decode_fails_on_nonzero_length() {
        let mut r = MqttBufReader::new(&ENCODED_NONZERO_LENGTH);
        assert_eq!(
            Pingresp::read(&mut r),
            Err(PacketReadError::MalformedPacket)
        );
    }

    #[test]
    fn decode_fails_on_invalid_packet_type() {
        let mut r = MqttBufReader::new(&ENCODED_INCORRECT_PACKET_TYPE);
        assert_eq!(
            Pingresp::read(&mut r),
            Err(PacketReadError::InvalidPacketType)
        );
    }
}
