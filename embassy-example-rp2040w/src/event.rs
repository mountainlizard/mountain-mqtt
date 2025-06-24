use mountain_mqtt::{client::EventHandlerError, packets::publish::ApplicationMessage};
use mountain_mqtt_embassy::mqtt_manager::FromApplicationMessage;

pub const TOPIC_LED: &str = "embassy-example-rp2040w-led";

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, defmt::Format)]
pub enum Event {
    Led(bool),
}

impl<const P: usize> FromApplicationMessage<P> for Event {
    fn from_application_message(
        message: &ApplicationMessage<P>,
    ) -> Result<Self, EventHandlerError> {
        message.try_into()
    }
}

impl<const P: usize> TryFrom<&ApplicationMessage<'_, P>> for Event {
    type Error = EventHandlerError;

    fn try_from(message: &ApplicationMessage<P>) -> Result<Self, Self::Error> {
        let received = match message.topic_name {
            TOPIC_LED => {
                let state = parse_led(message.payload)?;
                Ok(Self::Led(state))
            }
            _ => Err(EventHandlerError::UnexpectedApplicationMessageTopic),
        }?;

        Ok(received)
    }
}

fn parse_led(payload: &[u8]) -> Result<bool, EventHandlerError> {
    let mut string_unescape_buffer = [0u8; 64];
    let (state, _) = serde_json_core::from_slice_escaped(payload, &mut string_unescape_buffer)
        .map_err(|_e| EventHandlerError::InvalidApplicationMessage)?;
    Ok(state)
}
