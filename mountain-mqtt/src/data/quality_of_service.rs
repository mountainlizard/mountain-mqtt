use core::fmt::{Display, Formatter};

use crate::error::PacketReadError;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug, Eq, Hash, PartialOrd)]
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

#[cfg(feature = "defmt")]
impl defmt::Format for QualityOfService {
    fn format(&self, f: defmt::Formatter) {
        match self {
            QualityOfService::Qos0 => defmt::write!(f, "QoS0"),
            QualityOfService::Qos1 => defmt::write!(f, "QoS1"),
            QualityOfService::Qos2 => defmt::write!(f, "QoS2"),
        }
    }
}

impl Display for QualityOfService {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            QualityOfService::Qos0 => write!(f, "QoS0"),
            QualityOfService::Qos1 => write!(f, "QoS1"),
            QualityOfService::Qos2 => write!(f, "QoS2"),
        }
    }
}
