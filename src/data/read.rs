use super::mqtt_reader::{MqttReader, Result};

pub trait Read<'a> {
    fn read<R: MqttReader<'a>>(reader: &mut R) -> Result<Self>
    where
        Self: Sized;
}

impl<'a> Read<'a> for u8 {
    fn read<R: MqttReader<'a>>(reader: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        reader.get_u8()
    }
}

impl<'a> Read<'a> for u16 {
    fn read<R: MqttReader<'a>>(reader: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        reader.get_u16()
    }
}

impl<'a> Read<'a> for u32 {
    fn read<R: MqttReader<'a>>(reader: &mut R) -> Result<Self>
    where
        Self: Sized,
    {
        reader.get_u32()
    }
}

// impl<'a> Read<'a> for &'a str {
//     fn read<R: MqttReader<'a>>(reader: &mut R) -> Result<Self>
//     where
//         Self: Sized,
//     {
//         reader.get_str()
//     }
// }
