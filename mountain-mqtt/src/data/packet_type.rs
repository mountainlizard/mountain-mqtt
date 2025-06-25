use crate::{error::PacketReadError, packets::publish::is_valid_publish_first_header_byte};

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum PacketType {
    /// Connection request
    /// Client to Server
    Connect = 1,
    /// Connect acknowledgment
    /// Server to Client
    Connack = 2,
    /// Publish message
    /// Client to Server or Server to Client
    Publish = 3,
    /// Publish acknowledgment (quality of service 1)
    /// Client to Server or Server to Client
    Puback = 4,
    /// Publish received (quality of service 2 delivery part 1)
    /// Client to Server or Server to Client
    Pubrec = 5,
    /// Publish release (quality of service 2 delivery part 2)
    /// Client to Server or Server to Client
    Pubrel = 6,
    /// Publish complete (quality of service 2 delivery part 3)
    /// Client to Server or Server to Client
    Pubcomp = 7,
    /// Subscribe request
    /// Client to Server
    Subscribe = 8,
    /// Subscribe acknowledgment
    /// Server to Client
    Suback = 9,
    /// Unsubscribe request
    /// Client to Server
    Unsubscribe = 10,
    /// Unsubscribe acknowledgment
    /// Server to Client
    Unsuback = 11,
    /// PING request
    /// Client to Server
    Pingreq = 12,
    /// PING response
    /// Server to Client
    Pingresp = 13,
    /// Disconnect notification
    /// Client to Server or Server to Client
    Disconnect = 14,
    /// Authentication exchange
    /// Client to Server or Server to Client
    Auth = 15,
}

impl PacketType {
    /// Check whether a u8 value is a valid first header byte of a packet
    pub fn is_valid_first_header_byte(encoded: u8) -> bool {
        if let Ok(packet_type) = PacketType::try_from(encoded) {
            match packet_type {
                PacketType::Publish => is_valid_publish_first_header_byte(encoded),
                _ => encoded == packet_type.into(),
            }
        } else {
            false
        }
    }
}

/// Note this only provides the "base" representation, with no
/// additional flags set for [PacketType::Publish], but does
/// include the fixed additional bits that some packets require.
impl From<PacketType> for u8 {
    fn from(value: PacketType) -> Self {
        match value {
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
        }
    }
}

/// Parse the [PacketType] from a [u8], using only the upper 4 bits
/// and ignoring any additional flags set for [PacketType::Publish]
impl TryFrom<u8> for PacketType {
    type Error = PacketReadError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value & 0xF0 {
            0x10 => Ok(PacketType::Connect),
            0x20 => Ok(PacketType::Connack),
            0x30 => Ok(PacketType::Publish),
            0x40 => Ok(PacketType::Puback),
            0x50 => Ok(PacketType::Pubrec),
            0x60 => Ok(PacketType::Pubrel),
            0x70 => Ok(PacketType::Pubcomp),
            0x80 => Ok(PacketType::Subscribe),
            0x90 => Ok(PacketType::Suback),
            0xA0 => Ok(PacketType::Unsubscribe),
            0xB0 => Ok(PacketType::Unsuback),
            0xC0 => Ok(PacketType::Pingreq),
            0xD0 => Ok(PacketType::Pingresp),
            0xE0 => Ok(PacketType::Disconnect),
            0xF0 => Ok(PacketType::Auth),
            _ => Err(PacketReadError::InvalidPacketType),
        }
    }
}
