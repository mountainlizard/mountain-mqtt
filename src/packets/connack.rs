use super::{
    packet::PacketWrite, packet_type::PacketType, property::ConnackProperty,
    reason_code::ReasonCode,
};
use crate::data::{
    mqtt_writer::{self, MqttLenWriter, MqttWriter},
    write::Write,
};
use heapless::Vec;

pub struct Connack<'a, const PROPERTIES_N: usize> {
    session_present: bool,
    connect_reason_code: ReasonCode,
    properties: Vec<ConnackProperty<'a>, PROPERTIES_N>,
}

impl<'a, const PROPERTIES_N: usize> Connack<'a, PROPERTIES_N> {
    pub fn new(session_present: bool, connect_reason_code: ReasonCode) -> Self {
        Self {
            session_present,
            connect_reason_code,
            properties: Vec::new(),
        }
    }

    pub fn push_property(&mut self, p: ConnackProperty<'a>) -> Result<(), ConnackProperty<'a>> {
        self.properties.push(p)
    }

    fn write_properties<'w, W: MqttWriter<'w>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        for p in self.properties.iter() {
            p.write(writer)?;
        }
        Ok(())
    }
}

impl<const PROPERTIES_N: usize> PacketWrite for Connack<'_, PROPERTIES_N> {
    fn packet_type(&self) -> PacketType {
        PacketType::Connack
    }

    fn write_variable_header<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()> {
        // Find length of written properties
        let mut lw = MqttLenWriter::new();
        self.write_properties(&mut lw)?;
        let properties_len = lw.position();

        // Write the fixed parts of the variable header
        let connack_flags = if self.session_present { 1 } else { 0 };
        writer.put_u8(connack_flags)?; // 3.2.2.1 Connect Acknowledge Flags
        writer.put_u8(self.connect_reason_code as u8)?; // 3.2.2.2 Connect Reason Code

        // Write the properties array
        writer.put_variable_u32(properties_len as u32)?; // 3.2.2.3.1 Property Length
        self.write_properties(writer)?; // 3.2.2.3.2 onwards, properties

        Ok(())
    }

    fn write_payload<'w, W: MqttWriter<'w>>(&self, _writer: &mut W) -> mqtt_writer::Result<()> {
        Ok(())
    }
}
