use super::mqtt_reader::{MqttReader, Result};

pub trait Read<'a> {
    fn read<R: MqttReader<'a>>(reader: &mut R) -> Result<Self>
    where
        Self: Sized;
}
