use heapless::Vec;
use mountain_mqtt::packets::connect::{Connect, Will};

#[test]
fn create_connect_packet_with_will() {
    Connect::<0>::new(
        60,
        Some("user"),
        Some("password".as_bytes()),
        "client_id",
        true,
        Some(Will::new(
            mountain_mqtt::data::quality_of_service::QualityOfService::Qos0,
            false,
            "topic_name",
            "payload".as_bytes(),
            Vec::new(),
        )),
        Vec::new(),
    );
}
