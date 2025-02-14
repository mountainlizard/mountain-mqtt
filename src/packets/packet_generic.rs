use super::{
    auth::Auth, connack::Connack, connect::Connect, disconnect::Disconnect, pingreq::Pingreq,
    pingresp::Pingresp, puback::Puback, pubcomp::Pubcomp, publish::Publish, pubrec::Pubrec,
    pubrel::Pubrel, suback::Suback, subscribe::Subscribe, unsuback::Unsuback,
    unsubscribe::Unsubscribe,
};

#[derive(Debug, PartialEq)]
pub enum PacketGeneric<'a, const PROPERTIES_N: usize, const REQUEST_N: usize> {
    Connect(Connect<'a, PROPERTIES_N>),
    Connack(Connack<'a, PROPERTIES_N>),
    Publish(Publish<'a, PROPERTIES_N>),
    Puback(Puback<'a, PROPERTIES_N>),
    Pubrec(Pubrec<'a, PROPERTIES_N>),
    Pubrel(Pubrel<'a, PROPERTIES_N>),
    Pubcomp(Pubcomp<'a, PROPERTIES_N>),
    Subscribe(Subscribe<'a, PROPERTIES_N, REQUEST_N>),
    Suback(Suback<'a, PROPERTIES_N, REQUEST_N>),
    Unsubscribe(Unsubscribe<'a, PROPERTIES_N, REQUEST_N>),
    Unsuback(Unsuback<'a, PROPERTIES_N, REQUEST_N>),
    Pingreq(Pingreq),
    Pingresp(Pingresp),
    Disconnect(Disconnect<'a, PROPERTIES_N>),
    Auth(Auth<'a, PROPERTIES_N>),
}
