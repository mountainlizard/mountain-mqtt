use crate::codec::mqtt_reader::PacketReadError;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum QualityOfService {
    QoS0 = 0,
    QoS1 = 1,
    QoS2 = 2,
}

impl TryFrom<u8> for QualityOfService {
    type Error = PacketReadError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(QualityOfService::QoS0),
            1 => Ok(QualityOfService::QoS1),
            2 => Ok(QualityOfService::QoS2),
            _ => Err(PacketReadError::InvalidQoSValue),
        }
    }
}
