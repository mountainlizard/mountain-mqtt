use crate::data::quality_of_service::QualityOfService;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct PacketIdentifier(pub u16);

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
