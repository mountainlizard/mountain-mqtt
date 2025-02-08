use super::mqtt_writer::{MqttWriter, Result};

pub trait Write {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> Result<()>;
}
