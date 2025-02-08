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
    /// Publish acknowledgment (QoS 1)
    /// Client to Server or Server to Client
    Puback = 4,
    /// Publish received (QoS 2 delivery part 1)
    /// Client to Server or Server to Client
    Pubrec = 5,
    /// Publish release (QoS 2 delivery part 2)
    /// Client to Server or Server to Client
    Pubrel = 6,
    /// Publish complete (QoS 2 delivery part 3)
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
