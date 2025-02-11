use super::{
    packet::{Packet, PacketWrite},
    packet_type::PacketType,
    property::ConnackProperty,
    reason_code::ReasonCode,
};
use crate::data::{
    mqtt_reader::{self, MqttReaderError},
    mqtt_writer::{self, MqttWriter},
    read::Read,
};
use heapless::Vec;

pub struct Connack<'a, const PROPERTIES_N: usize> {
    session_present: bool,
    connect_reason_code: ReasonCode,
    properties: Vec<ConnackProperty<'a>, PROPERTIES_N>,
}

impl<const PROPERTIES_N: usize> Connack<'_, PROPERTIES_N> {
    pub fn new(session_present: bool, connect_reason_code: ReasonCode) -> Self {
        Self {
            session_present,
            connect_reason_code,
            properties: Vec::new(),
        }
    }
}

impl<const PROPERTIES_N: usize> Packet for Connack<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Connack
    }
}

// TODO: combine variable header and payload, doesn't seem like there's really a meaningful distinction in terms of handling?

impl<const PROPERTIES_N: usize> PacketWrite for Connack<'_, PROPERTIES_N> {
    fn write_variable_header<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Write the fixed parts of the variable header
        // Note this byte is technically "connack_flags", but only contains one bit of data, which
        // is encoded the same way as a bool-zero-one used for other data
        writer.put_bool_zero_one(self.session_present)?; // 3.2.2.1 Connect Acknowledge Flags
        writer.put_u8(self.connect_reason_code as u8)?; // 3.2.2.2 Connect Reason Code

        // Write the properties vec (3.2.2.3)
        writer.put_variable_u32_delimited_vec(&self.properties)?;

        Ok(())
    }

    fn write_payload<'w, W: MqttWriter<'w>>(&self, _writer: &mut W) -> mqtt_writer::Result<()> {
        Ok(())
    }
}

// TODO: Add PacketRead: Packet - it should accept a reader and the "special" bits of the first byte header that aren't used for packet type, and produce a packet. This can be used
// to implement a generic packet read that gets the packet type and special bits, decodes the length, limits the reader to length (if we want to), then passes over to PacketRead to do the rest.
// Also can be used in code to decode an unknown packet type by first reading the first byte header, then matching to dispatch to the right Read trait, then wrapping in "newtype enum", e.g. "PacketGeneric".

impl<'a, const PROPERTIES_N: usize> Read<'a> for Connack<'a, PROPERTIES_N> {
    fn read<R: crate::data::mqtt_reader::MqttReader<'a>>(
        reader: &mut R,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        let first_header_byte = reader.get_u8()?;
        let packet_type = PacketType::try_from(first_header_byte)
            .map_err(|_e| MqttReaderError::MalformedPacket)?;

        if packet_type != PacketType::Connack {
            return Err(MqttReaderError::IncorrectPacketType);
        }

        let remaining_length = reader.get_variable_u32()? as usize;
        let packet_end_position = reader.position() + remaining_length;

        // Note this byte is technically "connack_flags", but only contains one bit of data, which
        // is encoded the same way as a bool-zero-one used for other data
        let session_present = reader.get_bool_zero_one()?;

        let connect_reason_code = reader.get_reason_code()?;

        let mut packet: Connack<'a, PROPERTIES_N> =
            Connack::new(session_present, connect_reason_code);

        reader.get_variable_u32_delimited_vec(&mut packet.properties)?;

        // Check remaining length was correct
        if reader.position() == packet_end_position {
            Ok(packet)
        } else {
            Err(MqttReaderError::MalformedPacket)
        }
    }
}
