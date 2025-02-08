use crate::error::Error;

use super::packet_type::PacketType;

pub struct FixedHeaderFirstByte(u8);

impl From<PacketType> for FixedHeaderFirstByte {
    fn from(value: PacketType) -> Self {
        let byte = match value {
            PacketType::Connect => 0x10,
            PacketType::Connack => 0x20,
            PacketType::Publish => 0x30,
            PacketType::Puback => 0x40,
            PacketType::Pubrec => 0x50,
            PacketType::Pubrel => 0x62,
            PacketType::Pubcomp => 0x70,
            PacketType::Subscribe => 0x82,
            PacketType::Suback => 0x90,
            PacketType::Unsubscribe => 0xA2,
            PacketType::Unsuback => 0xB0,
            PacketType::Pingreq => 0xC0,
            PacketType::Pingresp => 0xD0,
            PacketType::Disconnect => 0xE0,
            PacketType::Auth => 0xF0,
        };
        FixedHeaderFirstByte(byte)
    }
}

impl TryFrom<FixedHeaderFirstByte> for PacketType {
    type Error = Error;

    fn try_from(value: FixedHeaderFirstByte) -> Result<Self, Self::Error> {
        match value.0 {
            0x10 => Ok(PacketType::Connect),
            0x20 => Ok(PacketType::Connack),
            0x30 => Ok(PacketType::Publish),
            0x40 => Ok(PacketType::Puback),
            0x50 => Ok(PacketType::Pubrec),
            0x62 => Ok(PacketType::Pubrel),
            0x70 => Ok(PacketType::Pubcomp),
            0x82 => Ok(PacketType::Subscribe),
            0x90 => Ok(PacketType::Suback),
            0xA2 => Ok(PacketType::Unsubscribe),
            0xB0 => Ok(PacketType::Unsuback),
            0xC0 => Ok(PacketType::Pingreq),
            0xD0 => Ok(PacketType::Pingresp),
            0xE0 => Ok(PacketType::Disconnect),
            0xF0 => Ok(PacketType::Auth),
            _ => Err(Error::InvalidFixedHeaderFirstByte),
        }
    }
}
