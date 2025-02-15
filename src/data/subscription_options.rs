use crate::error::PacketReadError;

use super::quality_of_service::QualityOfService;

#[derive(Debug, PartialEq, Copy, Clone)]
#[repr(u8)]
pub enum RetainHandling {
    SendOnSubscribe = 0,
    SendOnNewSubscribe = 1,
    DoNotSend = 2,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct SubscriptionOptions {
    pub maximum_qos: QualityOfService,
    pub no_local: bool,
    pub retain_as_published: bool,
    pub retain_handling: RetainHandling,
}

const QOS_MASK: u8 = 0x3;
const NO_LOCAL_BIT: u8 = 1 << 2;
const RETAIN_AS_PUBLISHED_BIT: u8 = 1 << 3;
const RETAIN_HANDLING_SHIFT: i32 = 4;
const RETAIN_HANDLING_MASK: u8 = 0x3;

impl TryFrom<u8> for SubscriptionOptions {
    type Error = PacketReadError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let maximum_qos_value = value & QOS_MASK;
        let maximum_qos = maximum_qos_value.try_into()?;
        let no_local = value & (NO_LOCAL_BIT) != 0;
        let retain_as_published = value & (RETAIN_AS_PUBLISHED_BIT) != 0;
        let retain_handling = match (value >> RETAIN_HANDLING_SHIFT) & RETAIN_HANDLING_MASK {
            0 => Ok(RetainHandling::SendOnSubscribe),
            1 => Ok(RetainHandling::SendOnNewSubscribe),
            2 => Ok(RetainHandling::DoNotSend),
            _ => Err(PacketReadError::MalformedPacket),
        }?;
        Ok(SubscriptionOptions {
            maximum_qos,
            no_local,
            retain_as_published,
            retain_handling,
        })
    }
}

impl From<&SubscriptionOptions> for u8 {
    fn from(value: &SubscriptionOptions) -> Self {
        let mut encoded = value.maximum_qos as u8;
        if value.no_local {
            encoded |= NO_LOCAL_BIT;
        }
        if value.retain_as_published {
            encoded |= RETAIN_AS_PUBLISHED_BIT;
        }
        encoded |= (value.retain_handling as u8) << RETAIN_HANDLING_SHIFT;

        encoded
    }
}
