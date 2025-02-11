use crate::data::{
    mqtt_writer::{self, MqttLenWriter, MqttWriter},
    write::Write,
};

use super::packet_type::PacketType;

pub const KEEP_ALIVE_DEFAULT: u16 = 60;
pub const PROTOCOL_NAME: &str = "MQTT";
pub const PROTOCOL_VERSION_5: u8 = 0x05;

pub trait Packet {
    fn packet_type(&self) -> PacketType;
    fn fixed_header_first_byte(&self) -> u8 {
        self.packet_type().into()
    }
}

pub trait PacketWrite: Packet {
    fn write_variable_header<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()>;

    fn write_payload<'w, W: MqttWriter<'w>>(&self, writer: &mut W) -> mqtt_writer::Result<()>;
}

impl<P: PacketWrite> Write for P {
    fn write<'w, W: MqttWriter<'w>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        // Find length of variable header, and payload
        let mut lw = MqttLenWriter::new();
        self.write_variable_header(&mut lw)?;
        self.write_payload(&mut lw)?;
        let remaining_length = lw.position();

        // fixed header including length, then variable header and payload
        writer.put_u8(self.fixed_header_first_byte())?;
        writer.put_variable_u32(remaining_length as u32)?;
        self.write_variable_header(writer)?;
        self.write_payload(writer)?;

        Ok(())
    }
}
