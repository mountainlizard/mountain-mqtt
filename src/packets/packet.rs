use crate::codec::{
    mqtt_reader::{self, MqttReader, MqttReaderError},
    mqtt_writer::{self, MqttLenWriter, MqttWriter},
    read::Read,
    write::Write,
};
use crate::data::packet_type::PacketType;

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
    fn put_variable_header_and_payload<'w, W: MqttWriter<'w>>(
        &self,
        writer: &mut W,
    ) -> mqtt_writer::Result<()>;
}

impl<P: PacketWrite> Write for P {
    fn write<'w, W: MqttWriter<'w>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        // Find length of variable header, and payload
        let mut lw = MqttLenWriter::new();
        self.put_variable_header_and_payload(&mut lw)?;
        let remaining_length = lw.position();

        // fixed header including length, then variable header and payload
        writer.put_u8(self.fixed_header_first_byte())?;
        writer.put_variable_u32(remaining_length as u32)?;
        self.put_variable_header_and_payload(writer)?;

        Ok(())
    }
}

pub trait PacketRead<'a>: Packet {
    /// Read the variable header and payload from a reader
    /// Note that the reader is NOT necessarily at position 0 when provided,
    /// so implementations must check position if needed.
    /// The first header byte is provided - this only needs to be used if
    /// it may contain information in addition to the [PacketType] -
    /// since this trait extends [Packet] it's required that
    /// any user of this trait independently checks that the
    /// first header byte matches the expected [PacketType] before
    /// calling this method.
    /// `len` is the remaining length read from the fixed header,
    /// and so is the amount of data expected to be present in the
    /// reader and needed to decode the variable header and payload.
    /// As noted above, the reader position may not begin at 0, so
    /// if you need to track the amount of data read against len,
    /// make sure to check the reader position before using it.
    fn get_variable_header_and_payload<R: MqttReader<'a>>(
        reader: &mut R,
        first_header_byte: u8,
        len: usize,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized;
}

impl<'a, P: PacketRead<'a>> Read<'a> for P {
    fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        let first_header_byte = reader.get_u8()?;

        let remaining_length = reader.get_variable_u32()? as usize;
        let packet_end_position = reader.position() + remaining_length;

        let packet = <Self as PacketRead>::get_variable_header_and_payload(
            reader,
            first_header_byte,
            remaining_length,
        )?;

        // Check that packet type is as expected, so `PacketRead` implementation
        // doesn't have to
        let packet_type = PacketType::try_from(first_header_byte)
            .map_err(|_e| MqttReaderError::MalformedPacket)?;
        if packet_type != packet.packet_type() {
            return Err(MqttReaderError::IncorrectPacketType);
        }

        // Check remaining length was correct
        if reader.position() == packet_end_position {
            Ok(packet)
        } else {
            Err(MqttReaderError::MalformedPacket)
        }
    }
}
