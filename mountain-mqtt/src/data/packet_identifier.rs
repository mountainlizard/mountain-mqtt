use crate::data::quality_of_service::QualityOfService;

#[derive(Debug, PartialEq, Copy, Clone, Eq, Hash, Default)]
pub struct PacketIdentifier(pub u16);

impl PacketIdentifier {
    /// Increment this packet identifier to the next identifier in sequence,
    /// wrapping if this overflows
    pub fn increment_wrapping(&mut self) {
        self.0 = self.0.wrapping_add(1)
    }
}

#[derive(Debug, PartialEq)]
pub enum PublishPacketIdentifier {
    None,
    Qos1(PacketIdentifier),
    Qos2(PacketIdentifier),
}

impl PublishPacketIdentifier {
    pub fn qos(&self) -> QualityOfService {
        match self {
            PublishPacketIdentifier::None => QualityOfService::Qos0,
            PublishPacketIdentifier::Qos1(_id) => QualityOfService::Qos1,
            PublishPacketIdentifier::Qos2(_id) => QualityOfService::Qos2,
        }
    }
}
