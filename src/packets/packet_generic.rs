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

/// A generic packet, this has a variant for each packet type
/// Allows for e.g. decoding data of an unknown packet type, we can
/// then match to handle the different cases.
/// `P` is the maximum number of properties in a packet.
/// `S` is the maximum number of _additional_ subscription requests
/// after the mandatory request.
#[derive(Debug, PartialEq)]
pub enum PacketGeneric<'a, const P: usize, const S: usize> {
    Connect(Connect<'a, P>),
    Connack(Connack<'a, P>),
    Publish(Publish<'a, P>),
    Puback(Puback<'a, P>),
    Pubrec(Pubrec<'a, P>),
    Pubrel(Pubrel<'a, P>),
    Pubcomp(Pubcomp<'a, P>),
    Subscribe(Subscribe<'a, P, S>),
    Suback(Suback<'a, P, S>),
    Unsubscribe(Unsubscribe<'a, P, S>),
    Unsuback(Unsuback<'a, P, S>),
    Pingreq(Pingreq),
    Pingresp(Pingresp),
    Disconnect(Disconnect<'a, P>),
    Auth(Auth<'a, P>),
}

impl<'a, const P: usize, const S: usize> Read<'a> for PacketGeneric<'a, P, S> {
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
            Err(PacketReadError::IncorrectPacketLength)
        }
    }
}

impl<const P: usize, const S: usize> Packet for PacketGeneric<'_, P, S> {
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
