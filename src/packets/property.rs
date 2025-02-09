use crate::data::{
    mqtt_reader::{self, MqttReader, MqttReaderError},
    mqtt_writer::{self, MqttWriter},
    read::Read,
    write::Write,
};

use super::string_pair::StringPair;

use core::marker::PhantomData;
pub trait Property<'a, T> {
    const IDENTIFIER: u32;
    fn value(&self) -> T;
}

#[macro_export]
macro_rules! property_u8 {
    ( $n:ident, $c:literal ) => {
        #[derive(Debug, PartialEq)]
        pub struct $n<'a> {
            value: u8,
            phantom: PhantomData<&'a u8>,
        }

        impl<'a> $n<'a> {
            pub fn new(value: u8) -> Self {
                Self {
                    value,
                    phantom: PhantomData,
                }
            }
        }

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_u8(self.value)
            }
        }

        impl<'a> Read<'a> for $n<'_> {
            fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                let value = reader.get_u8()?;
                Ok(Self::new(value))
            }
        }

        impl<'a> Property<'a, u8> for $n<'a> {
            const IDENTIFIER: u32 = $c;
            fn value(&self) -> u8 {
                self.value
            }
        }
    };
}

// #[macro_export]
// macro_rules! property_u16 {
//     ( $n:ident, $c:literal ) => {
//         pub struct $n(u16);

//         impl Write for $n {
//             fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
//                 writer.put_u16(self.0)
//             }
//         }

//         impl<'a> Read<'a> for $n {
//             fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
//             where
//                 Self: Sized,
//             {
//                 Ok($n(reader.get_u16()?))
//             }
//         }

//         impl Property for $n {
//             const IDENTIFIER: u32 = $c;
//         }
//     };
// }

// #[macro_export]
// macro_rules! property_u32 {
//     ( $n:ident, $c:literal ) => {
//         pub struct $n(u32);

//         impl Write for $n {
//             fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
//                 writer.put_u32(self.0)
//             }
//         }

//         impl<'a> Read<'a> for $n {
//             fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
//             where
//                 Self: Sized,
//             {
//                 Ok($n(reader.get_u32()?))
//             }
//         }

//         impl Property for $n {
//             const IDENTIFIER: u32 = $c;
//         }
//     };
// }

// #[macro_export]
// macro_rules! property_variable_u32 {
//     ( $n:ident, $c:literal ) => {
//         pub struct $n(u32);

//         impl Write for $n {
//             fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
//                 writer.put_variable_u32(self.0)
//             }
//         }

//         impl<'a> Read<'a> for $n {
//             fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
//             where
//                 Self: Sized,
//             {
//                 Ok($n(reader.get_variable_u32()?))
//             }
//         }

//         impl Property for $n {
//             const IDENTIFIER: u32 = $c;
//         }
//     };
// }

// #[macro_export]
// macro_rules! property_str {
//     ( $n:ident, $c:literal ) => {
//         pub struct $n<'a>(&'a str);

//         impl Write for $n<'_> {
//             fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
//                 writer.put_str(self.0)
//             }
//         }

//         impl<'a> Read<'a> for $n<'a> {
//             fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
//             where
//                 Self: Sized,
//             {
//                 Ok($n(reader.get_str()?))
//             }
//         }

//         impl Property for $n<'_> {
//             const IDENTIFIER: u32 = $c;
//         }
//     };
// }

// #[macro_export]
// macro_rules! property_string_pair {
//     ( $n:ident, $c:literal ) => {
//         pub struct $n<'a>(StringPair<'a>);

//         impl Write for $n<'_> {
//             fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
//                 writer.put_string_pair(&self.0)
//             }
//         }

//         impl<'a> Read<'a> for $n<'a> {
//             fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
//             where
//                 Self: Sized,
//             {
//                 Ok($n(reader.get_string_pair()?))
//             }
//         }

//         impl Property for $n<'_> {
//             const IDENTIFIER: u32 = $c;
//         }
//     };
// }

// #[macro_export]
// macro_rules! property_binary_data {
//     ( $n:ident, $c:literal ) => {
//         pub struct $n<'a>(&'a [u8]);

//         impl Write for $n<'_> {
//             fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
//                 writer.put_binary_data(self.0)
//             }
//         }

//         impl<'a> Read<'a> for $n<'a> {
//             fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
//             where
//                 Self: Sized,
//             {
//                 Ok($n(reader.get_binary_data()?))
//             }
//         }

//         impl Property for $n<'_> {
//             const IDENTIFIER: u32 = $c;
//         }
//     };
// }

// Macros define the struct for each property type, and implement Read and Write for it

property_u8!(PayloadFormatIndicator, 0x01);
// property_u32!(MessageExpiryInterval, 0x02);
// property_str!(ContentType, 0x03);
// property_str!(ResponseTopic, 0x08);
// property_binary_data!(CorrelationData, 0x09);
// property_variable_u32!(SubscriptionIdentifier, 0x0B);
// property_u32!(SessionExpiryInterval, 0x11);
// property_str!(AssignedClientIdentifier, 0x12);
// property_u16!(ServerKeepAlive, 0x13);
// property_str!(AuthenticationMethod, 0x15);
// property_binary_data!(AuthenticationData, 0x16);
property_u8!(RequestProblemInformation, 0x17);
// property_u32!(WillDelayInterval, 0x18);
property_u8!(RequestResponseInformation, 0x19);
// property_str!(ResponseInformation, 0x1A);
// property_str!(ServerReference, 0x1C);
// property_str!(ReasonString, 0x1F);
// property_u16!(ReceiveMaximum, 0x21);
// property_u16!(TopicAliasMaximum, 0x22);
// property_u16!(TopicAlias, 0x23);
property_u8!(MaximumQoS, 0x24);
property_u8!(RetainAvailable, 0x25);
// property_string_pair!(UserProperty, 0x26);
// property_u32!(MaximumPacketSize, 0x27);
property_u8!(WildcardSubscriptionAvailable, 0x28);
property_u8!(SubscriptionIdentifierAvailable, 0x29);
property_u8!(SharedSubscriptionAvailable, 0x2A);

// pub enum ConnectionProperty<'a> {
//     SessionExpiryInterval(SessionExpiryInterval),
//     ReceiveMaximum(ReceiveMaximum),
//     MaximumPacketSize(MaximumPacketSize),
//     TopicAliasMaximum(TopicAliasMaximum),
//     RequestResponseInformation(RequestResponseInformation),
//     RequestProblemInformation(RequestProblemInformation),
//     UserProperty(UserProperty<'a>),
//     AuthenticationMethod(AuthenticationMethod<'a>),
//     AuthenticationData(AuthenticationData<'a>),
// }

#[macro_export]
macro_rules! packet_properties {
    ( $n:ident, { $( $p:ident ),+ } ) => {

        #[derive(Debug, PartialEq)]
        pub enum $n<'a>{
            $(
                $p($p<'a>),
            )*
        }

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                match self {
                    $(
                        Self::$p(v) => {
                            writer.put_variable_u32($p::IDENTIFIER)?;
                            v.write(writer)?;
                            Ok(())
                        }
                    )*
                }
            }
        }

        impl<'a> Read<'a> for $n<'a> {
            fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                let id = reader.get_variable_u32()?;
                match id {
                    $(
                        $p::IDENTIFIER => {
                            let v = $p::read(reader)?;
                            Ok(Self::$p(v))
                        }
                    )*
                    _ => Err(MqttReaderError::MalformedPacket),
                }
            }
        }
    };
}

// impl Write for ConnectionProperty<'_> {
//     fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
//         match self {
//             Self::SessionExpiryInterval(v) => {
//                 writer.put_variable_u32(SessionExpiryInterval::IDENTIFIER)?;
//                 v.write(writer)?;
//                 Ok(())
//             }
//             Self::ReceiveMaximum(v) => todo!(),
//             Self::MaximumPacketSize(v) => todo!(),
//             Self::TopicAliasMaximum(v) => todo!(),
//             Self::RequestResponseInformation(v) => todo!(),
//             Self::RequestProblemInformation(v) => todo!(),
//             Self::UserProperty(v) => {
//                 writer.put_variable_u32(UserProperty::IDENTIFIER)?;
//                 v.write(writer)?;
//                 Ok(())
//             }
//             Self::AuthenticationMethod(v) => todo!(),
//             Self::AuthenticationData(v) => todo!(),
//         }
//     }
// }

// impl<'a> Read<'a> for ConnectionProperty<'a> {
//     fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
//     where
//         Self: Sized,
//     {
//         let id = reader.get_variable_u32()?;
//         match id {
//             SessionExpiryInterval::IDENTIFIER => {
//                 let v = SessionExpiryInterval::read(reader)?;
//                 Ok(Self::SessionExpiryInterval(v))
//             }
//             UserProperty::IDENTIFIER => {
//                 let v = UserProperty::read(reader)?;
//                 Ok(Self::UserProperty(v))
//             }
//             _ => Err(MqttReaderError::MalformedPacket),
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use crate::data::{mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter};

    use super::*;

    packet_properties!(SomePacketProperty, {
        PayloadFormatIndicator,
        RequestProblemInformation
    });

    #[test]
    fn write_a_property() {
        let mut buf = [0xFFu8; 4];
        let position = {
            let mut r = MqttBufWriter::new(&mut buf);

            let p = PayloadFormatIndicator::new(42);
            p.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(buf[0..position], [42u8]);
    }

    #[test]
    fn write_and_read_a_packet_property() {
        let mut buf = [0xFFu8; 4];

        let p = SomePacketProperty::PayloadFormatIndicator(PayloadFormatIndicator::new(42));

        let position = {
            let mut r = MqttBufWriter::new(&mut buf);

            p.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(
            buf[0..position],
            // Note the id is a variable byte integer - we know it encodes as one byte so just cast it
            [PayloadFormatIndicator::IDENTIFIER as u8, 42u8]
        );

        let mut r = MqttBufReader::new(&buf);
        let read_p = SomePacketProperty::read(&mut r).unwrap();
        assert_eq!(read_p, p);
    }
}
