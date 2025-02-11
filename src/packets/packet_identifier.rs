use super::quality_of_service::QualityOfService;

#[derive(Debug, PartialEq)]
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
            PublishPacketIdentifier::None => QualityOfService::QoS0,
            PublishPacketIdentifier::Qos1(_id) => QualityOfService::QoS1,
            PublishPacketIdentifier::Qos2(_id) => QualityOfService::QoS2,
        }
    }
}
