use crate::data::{
    mqtt_writer::{self, MqttWriter},
    write::Write,
};

use super::{packet::Packet, string_pair::StringPair};

// TODO: MACRO accepting just the data type and generating boilerplate Read/Write impls

pub struct PayloadFormatIndicator(u8);
pub struct MessageExpiryInterval(u32);
pub struct ContentType<'a>(&'a str);
pub struct ResponseTopic<'a>(&'a str);
pub struct CorrelationData<'a>(&'a [u8]);
pub struct SubscriptionIdentifier(u32);
pub struct SessionExpiryInterval(u32);
pub struct AssignedClientIdentifier<'a>(&'a str);
pub struct ServerKeepAlive(u16);
pub struct AuthenticationMethod<'a>(&'a str);
pub struct AuthenticationData<'a>(&'a [u8]);
pub struct RequestProblemInformation(u8);
pub struct WillDelayInterval(u32);
pub struct RequestResponseInformation(u8);
pub struct ResponseInformation<'a>(&'a str);
pub struct ServerReference<'a>(&'a str);
pub struct ReasonString<'a>(&'a str);
pub struct ReceiveMaximum(u16);
pub struct TopicAliasMaximum(u16);
pub struct TopicAlias(u16);
pub struct MaximumQoS(u8);
pub struct RetainAvailable(u8);
pub struct UserProperty<'a>(StringPair<'a>);
pub struct MaximumPacketSize(u32);
pub struct WildcardSubscriptionAvailable(u8);
pub struct SubscriptionIdentifierAvailable(u8);
pub struct SharedSubscriptionAvailable(u8);

impl Write for PayloadFormatIndicator {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        writer.put_u8(self.0)
    }
}

impl Write for MessageExpiryInterval {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        writer.put_u32(self.0)
    }
}

impl Write for ContentType<'_> {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        writer.put_str(self.0)
    }
}

impl Write for ResponseTopic<'_> {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        writer.put_str(self.0)
    }
}

impl Write for CorrelationData<'_> {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        writer.put_binary_data(self.0)
    }
}

impl Write for SubscriptionIdentifier {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        writer.put_variable_u32(self.0)
    }
}

impl Write for SessionExpiryInterval {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        writer.put_u32(self.0)
    }
}

impl Write for AssignedClientIdentifier<'_> {
    fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
        writer.put_str(self.0)
    }
}

pub enum ConnectionProperty<'a> {
    SessionExpiryInterval(SessionExpiryInterval),
    ReceiveMaximum(ReceiveMaximum),
    MaximumPacketSize(MaximumPacketSize),
    TopicAliasMaximum(TopicAliasMaximum),
    RequestResponseInformation(RequestResponseInformation),
    RequestProblemInformation(RequestProblemInformation),
    UserProperty(UserProperty<'a>),
    AuthenticationMethod(AuthenticationMethod<'a>),
    AuthenticationData(AuthenticationData<'a>),
}
