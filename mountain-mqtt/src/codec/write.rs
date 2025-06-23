use super::mqtt_writer::{MqttWriter, Result};

pub trait Write {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> Result<()>;
}

impl Write for u8 {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> Result<()> {
        writer.put_u8(*self)
    }
}

impl Write for u16 {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> Result<()> {
        writer.put_u16(*self)
    }
}

impl Write for u32 {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> Result<()> {
        writer.put_u32(*self)
    }
}

// impl<'b> Write for &'b str {
//     fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> Result<()> {
//         writer.put_str(self)
//     }
// }
