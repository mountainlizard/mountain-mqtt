use crate::{
    codec::{mqtt_reader, read::Read},
    data::packet_type::PacketType,
    error::PacketReadError,
};

use super::{
    auth::Auth,
    connack::Connack,
    connect::Connect,
    disconnect::Disconnect,
    packet::{Packet, PacketRead},
    pingreq::Pingreq,
    pingresp::Pingresp,
    puback::Puback,
    pubcomp::Pubcomp,
    publish::Publish,
    pubrec::Pubrec,
    pubrel::Pubrel,
    suback::Suback,
    subscribe::Subscribe,
    unsuback::Unsuback,
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

impl<'a, const PROPERTIES_N: usize, const REQUEST_N: usize> Read<'a>
    for PacketGeneric<'a, PROPERTIES_N, REQUEST_N>
{
    fn read<R: crate::codec::mqtt_reader::MqttReader<'a>>(
        reader: &mut R,
    ) -> mqtt_reader::Result<Self>
    where
        Self: Sized,
    {
        let first_header_byte = reader.get_u8()?;

        // Check that packet type is valid
        let packet_type = PacketType::try_from(first_header_byte)?;

        let len = reader.get_variable_u32()? as usize;
        let packet_end_position = reader.position() + len;

        let packet_generic = match packet_type {
            PacketType::Connect => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Connect(packet)
            }
            PacketType::Connack => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Connack(packet)
            }
            PacketType::Publish => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Publish(packet)
            }
            PacketType::Puback => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Puback(packet)
            }
            PacketType::Pubrec => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Pubrec(packet)
            }
            PacketType::Pubrel => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Pubrel(packet)
            }
            PacketType::Pubcomp => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Pubcomp(packet)
            }
            PacketType::Subscribe => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Subscribe(packet)
            }
            PacketType::Suback => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Suback(packet)
            }
            PacketType::Unsubscribe => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Unsubscribe(packet)
            }
            PacketType::Unsuback => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Unsuback(packet)
            }
            PacketType::Pingreq => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Pingreq(packet)
            }
            PacketType::Pingresp => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Pingresp(packet)
            }
            PacketType::Disconnect => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Disconnect(packet)
            }
            PacketType::Auth => {
                let packet =
                    PacketRead::get_variable_header_and_payload(reader, first_header_byte, len)?;
                PacketGeneric::Auth(packet)
            }
        };

        // Check remaining length was correct
        if reader.position() == packet_end_position {
            Ok(packet_generic)
        } else {
            Err(PacketReadError::MalformedPacket)
        }
    }
}

impl<const PROPERTIES_N: usize, const REQUEST_N: usize> Packet
    for PacketGeneric<'_, PROPERTIES_N, REQUEST_N>
{
    fn packet_type(&self) -> PacketType {
        match self {
            PacketGeneric::Connect(_) => PacketType::Connect,
            PacketGeneric::Connack(_) => PacketType::Connack,
            PacketGeneric::Publish(_) => PacketType::Publish,
            PacketGeneric::Puback(_) => PacketType::Puback,
            PacketGeneric::Pubrec(_) => PacketType::Pubrec,
            PacketGeneric::Pubrel(_) => PacketType::Pubrel,
            PacketGeneric::Pubcomp(_) => PacketType::Pubcomp,
            PacketGeneric::Subscribe(_) => PacketType::Subscribe,
            PacketGeneric::Suback(_) => PacketType::Suback,
            PacketGeneric::Unsubscribe(_) => PacketType::Unsubscribe,
            PacketGeneric::Unsuback(_) => PacketType::Unsuback,
            PacketGeneric::Pingreq(_) => PacketType::Pingreq,
            PacketGeneric::Pingresp(_) => PacketType::Pingresp,
            PacketGeneric::Disconnect(_) => PacketType::Disconnect,
            PacketGeneric::Auth(_) => PacketType::Auth,
        }
    }
}
