use crate::error::PacketReadError;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum QualityOfService {
    Qos0 = 0,
    Qos1 = 1,
    Qos2 = 2,
}

impl TryFrom<u8> for QualityOfService {
    type Error = PacketReadError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(QualityOfService::Qos0),
            1 => Ok(QualityOfService::Qos1),
            2 => Ok(QualityOfService::Qos2),
            _ => Err(PacketReadError::InvalidQosValue),
        }
    }
}
