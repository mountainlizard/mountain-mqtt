use super::{
    packet::{Packet, PacketRead, PacketWrite},
    packet_type::PacketType,
    property::ConnackProperty,
    reason_code::ReasonCode,
};
use crate::data::{
    mqtt_reader::{self, MqttReader},
    mqtt_writer::{self, MqttWriter},
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

impl<const PROPERTIES_N: usize> PacketWrite for Connack<'_, PROPERTIES_N> {
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Variable header:

        // Write the fixed parts of the variable header
        // Note this byte is technically "connack_flags", but only contains one bit of data, which
        // is encoded the same way as a bool-zero-one used for other data
        writer.put_bool_zero_one(self.session_present)?; // 3.2.2.1 Connect Acknowledge Flags
        writer.put_u8(self.connect_reason_code as u8)?; // 3.2.2.2 Connect Reason Code

        // Write the properties vec (3.2.2.3)
        writer.put_variable_u32_delimited_vec(&self.properties)?;

        // Payload: Empty

        Ok(())
    }
}

impl<'a, const PROPERTIES_N: usize> PacketRead<'a> for Connack<'a, PROPERTIES_N> {
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        _first_header_byte: u8,
        _len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        // Note this byte is technically "connack_flags", but only contains one bit of data, which
        // is encoded the same way as a bool-zero-one used for other data
        let session_present = reader.get_bool_zero_one()?;

        let connect_reason_code = reader.get_reason_code()?;

        let mut packet: Connack<'a, PROPERTIES_N> =
            Connack::new(session_present, connect_reason_code);

        // Add properties into packet
        reader.get_variable_u32_delimited_vec(&mut packet.properties)?;

        Ok(packet)
    }
}
