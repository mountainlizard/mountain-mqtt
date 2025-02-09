use crate::data::{
    mqtt_reader::{self, MqttReader},
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
macro_rules! property_owned {
    ( $n:ident, $t:ty, $c:literal ) => {
        #[derive(Debug, PartialEq)]
        pub struct $n<'a> {
            value: $t,
            phantom: PhantomData<&'a $t>,
        }

        impl<'a> $n<'a> {
            pub fn new(value: $t) -> Self {
                Self {
                    value,
                    phantom: PhantomData,
                }
            }
        }

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                self.value.write(writer)
            }
        }

        impl<'a> Read<'a> for $n<'_> {
            fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                let value = <$t as Read>::read(reader)?;
                Ok(Self::new(value))
            }
        }

        impl<'a> Property<'a, $t> for $n<'a> {
            const IDENTIFIER: u32 = $c;
            fn value(&self) -> $t {
                self.value
            }
        }

        impl From<$t> for $n<'_> {
            fn from(value: $t) -> Self {
                Self::new(value)
            }
        }
    };
}

#[macro_export]
macro_rules! property_variable_u32 {
    ( $n:ident, $c:literal ) => {
        #[derive(Debug, PartialEq)]
        pub struct $n<'a> {
            value: u32,
            phantom: PhantomData<&'a u32>,
        }

        impl<'a> $n<'a> {
            pub fn new(value: u32) -> Self {
                Self {
                    value,
                    phantom: PhantomData,
                }
            }
        }

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_variable_u32(self.value)
            }
        }

        impl<'a> Read<'a> for $n<'_> {
            fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                let value = reader.get_variable_u32()?;
                Ok(Self::new(value))
            }
        }

        impl<'a> Property<'a, u32> for $n<'a> {
            const IDENTIFIER: u32 = $c;
            fn value(&self) -> u32 {
                self.value
            }
        }

        impl From<u32> for $n<'_> {
            fn from(value: u32) -> Self {
                Self::new(value)
            }
        }
    };
}

#[macro_export]
macro_rules! property_str {
    ( $n:ident, $c:literal ) => {
        #[derive(Debug, PartialEq)]
        pub struct $n<'a> {
            value: &'a str,
        }

        impl<'a> $n<'a> {
            pub fn new(value: &'a str) -> Self {
                Self { value }
            }
        }

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_str(self.value)
            }
        }

        impl<'a> Read<'a> for $n<'a> {
            fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                let value = reader.get_str()?;
                Ok(Self::new(value))
            }
        }

        impl<'a> Property<'a, &'a str> for $n<'a> {
            const IDENTIFIER: u32 = $c;
            fn value(&self) -> &'a str {
                self.value
            }
        }

        impl<'a> From<&'a str> for $n<'a> {
            fn from(value: &'a str) -> Self {
                Self::new(value)
            }
        }
    };
}

#[macro_export]
macro_rules! property_string_pair {
    ( $n:ident, $c:literal ) => {
        #[derive(Debug, PartialEq)]
        pub struct $n<'a> {
            value: StringPair<'a>,
        }

        impl<'a> $n<'a> {
            pub fn new(value: StringPair<'a>) -> Self {
                Self { value }
            }
        }

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_string_pair(&self.value)
            }
        }

        impl<'a> Read<'a> for $n<'a> {
            fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                let value = reader.get_string_pair()?;
                Ok(Self::new(value))
            }
        }

        impl<'a> Property<'a, StringPair<'a>> for $n<'a> {
            const IDENTIFIER: u32 = $c;
            fn value(&self) -> StringPair<'a> {
                self.value
            }
        }

        impl<'a> From<StringPair<'a>> for $n<'a> {
            fn from(value: StringPair<'a>) -> Self {
                Self::new(value)
            }
        }
    };
}

#[macro_export]
macro_rules! property_binary_data {
    ( $n:ident, $c:literal ) => {
        #[derive(Debug, PartialEq)]
        pub struct $n<'a> {
            value: &'a [u8],
        }

        impl<'a> $n<'a> {
            pub fn new(value: &'a [u8]) -> Self {
                Self { value }
            }
        }

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_binary_data(self.value)
            }
        }

        impl<'a> Read<'a> for $n<'a> {
            fn read<R: MqttReader<'a>>(reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                let value = reader.get_binary_data()?;
                Ok(Self::new(value))
            }
        }

        impl<'a> Property<'a, &'a [u8]> for $n<'a> {
            const IDENTIFIER: u32 = $c;
            fn value(&self) -> &'a [u8] {
                self.value
            }
        }

        impl<'a> From<&'a [u8]> for $n<'a> {
            fn from(value: &'a [u8]) -> Self {
                Self::new(value)
            }
        }
    };
}

#[macro_export]
macro_rules! packet_properties {
    ( $n:ident, [ $( $p:ident ),+ ] ) => {

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
                    _ => Err($crate::packets::property::mqtt_reader::MqttReaderError::MalformedPacket),
                }
            }
        }
    };
}

// Macros define the struct for each property type, and implement Read and Write for it
property_owned!(PayloadFormatIndicator, u8, 0x01);
property_owned!(MessageExpiryInterval, u32, 0x02);
property_str!(ContentType, 0x03);
property_str!(ResponseTopic, 0x08);
property_binary_data!(CorrelationData, 0x09);
property_variable_u32!(SubscriptionIdentifier, 0x0B);
property_owned!(SessionExpiryInterval, u32, 0x11);
property_str!(AssignedClientIdentifier, 0x12);
property_owned!(ServerKeepAlive, u16, 0x13);
property_str!(AuthenticationMethod, 0x15);
property_binary_data!(AuthenticationData, 0x16);
property_owned!(RequestProblemInformation, u8, 0x17);
property_owned!(WillDelayInterval, u32, 0x18);
property_owned!(RequestResponseInformation, u8, 0x19);
property_str!(ResponseInformation, 0x1A);
property_str!(ServerReference, 0x1C);
property_str!(ReasonString, 0x1F);
property_owned!(ReceiveMaximum, u16, 0x21);
property_owned!(TopicAliasMaximum, u16, 0x22);
property_owned!(TopicAlias, u16, 0x23);
property_owned!(MaximumQoS, u8, 0x24);
property_owned!(RetainAvailable, u8, 0x25);
property_string_pair!(UserProperty, 0x26);
property_owned!(MaximumPacketSize, u32, 0x27);
property_owned!(WildcardSubscriptionAvailable, u8, 0x28);
property_owned!(SubscriptionIdentifierAvailable, u8, 0x29);
property_owned!(SharedSubscriptionAvailable, u8, 0x2A);

packet_properties!(
    ConnectionProperty,
    [
        SessionExpiryInterval,
        ReceiveMaximum,
        MaximumPacketSize,
        TopicAliasMaximum,
        RequestResponseInformation,
        RequestProblemInformation,
        UserProperty,
        AuthenticationMethod,
        AuthenticationData
    ]
);

#[cfg(test)]
mod tests {
    use crate::data::{mqtt_reader::MqttBufReader, mqtt_writer::MqttBufWriter};

    use super::*;

    property_owned!(PropertyU8, u8, 0x01);
    property_owned!(PropertyU16, u16, 0x02);
    property_owned!(PropertyU32, u32, 0x03);
    property_variable_u32!(PropertyVariableU32, 0x04);
    property_str!(PropertyString, 0x05);
    property_string_pair!(PropertyStringPair, 0x06);
    property_binary_data!(PropertyBinaryData, 0x07);

    packet_properties!(
        PacketAnyProperty,
        [
            PropertyU8,
            PropertyU16,
            PropertyU32,
            PropertyVariableU32,
            PropertyString,
            PropertyStringPair,
            PropertyBinaryData
        ]
    );

    packet_properties!(
        PacketFirstThreeProperty,
        [PropertyU8, PropertyU16, PropertyU32]
    );

    #[test]
    fn write_and_read_a_property_u8() {
        let mut buf = [0xFFu8; 4];
        let p = PropertyU8::new(42);
        let position = {
            let mut r = MqttBufWriter::new(&mut buf);

            p.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(buf[0..position], [42u8]);

        let mut r = MqttBufReader::new(&buf);
        let read_p = PropertyU8::read(&mut r).unwrap();
        assert_eq!(read_p, p);
    }

    #[test]
    fn write_and_read_a_packet_property_u8() {
        let mut buf = [0xFFu8; 4];

        let p = PacketAnyProperty::PropertyU8(PropertyU8::new(42));

        let position = {
            let mut r = MqttBufWriter::new(&mut buf);

            p.write(&mut r).unwrap();
            r.position()
        };
        assert_eq!(
            buf[0..position],
            // Note the id is a variable byte integer - we know it encodes as one byte so just cast it
            [PropertyU8::IDENTIFIER as u8, 42u8]
        );

        let mut r = MqttBufReader::new(&buf);
        let read_p = PacketAnyProperty::read(&mut r).unwrap();
        assert_eq!(read_p, p);
    }

    #[test]
    fn write_and_read_a_full_set_of_properties() {
        let mut buf = [0xFFu8; 1024];
        let data: &[u8] = &[1u8, 2, 3, 4, 5, 6];

        let properties = [
            PacketAnyProperty::PropertyU8(1.into()),
            PacketAnyProperty::PropertyU16(2.into()),
            PacketAnyProperty::PropertyU32(3.into()),
            PacketAnyProperty::PropertyVariableU32(4.into()),
            PacketAnyProperty::PropertyString("hello world".into()),
            PacketAnyProperty::PropertyStringPair(StringPair::new("name", "value").into()),
            PacketAnyProperty::PropertyBinaryData(data.into()),
        ];

        let position = {
            let mut r = MqttBufWriter::new(&mut buf);

            for p in properties.iter() {
                p.write(&mut r).unwrap();
            }
            r.position()
        };

        let mut r = MqttBufReader::new(&buf[0..position]);
        for p in properties.iter() {
            let p_read = PacketAnyProperty::read(&mut r).unwrap();
            assert_eq!(&p_read, p);
        }
    }

    #[test]
    fn write_and_read_expected_subset_of_properties_for_packet() {
        let mut buf = [0xFFu8; 1024];

        // These properties are from "PacketAny" that can accept any property,
        // but are also accepted by PacketFirstThreeProperty
        let properties = [
            PacketAnyProperty::PropertyU8(1.into()),
            PacketAnyProperty::PropertyU16(2.into()),
            PacketAnyProperty::PropertyU32(3.into()),
        ];

        let expected_properties = [
            PacketFirstThreeProperty::PropertyU8(1.into()),
            PacketFirstThreeProperty::PropertyU16(2.into()),
            PacketFirstThreeProperty::PropertyU32(3.into()),
        ];

        // Write out the properties as properties of PacketAnyProperty
        let position = {
            let mut r = MqttBufWriter::new(&mut buf);

            for p in properties.iter() {
                p.write(&mut r).unwrap();
            }
            r.position()
        };

        // Read back as PacketFirstThreeProperty properties, so we can check they work
        let mut r = MqttBufReader::new(&buf[0..position]);
        for p in expected_properties.iter() {
            let p_read = PacketFirstThreeProperty::read(&mut r).unwrap();
            assert_eq!(&p_read, p);
        }
    }

    #[test]
    fn fail_with_malformed_packet_on_reading_unexpected_properties_outside_subset_for_packet() {
        // These properties are in PacketAnyProperty but not in PacketFirstThreeProperty,
        // so reading any of them should fail
        let data: &[u8] = &[1u8, 2, 3, 4, 5, 6];
        let unexpected_properties = [
            PacketAnyProperty::PropertyVariableU32(4.into()),
            PacketAnyProperty::PropertyString("hello world".into()),
            PacketAnyProperty::PropertyStringPair(StringPair::new("name", "value").into()),
            PacketAnyProperty::PropertyBinaryData(data.into()),
        ];

        let mut buf = [0xFFu8; 1024];
        for p in unexpected_properties.iter() {
            buf.fill(0xFFu8);
            let position = {
                let mut r = MqttBufWriter::new(&mut buf);
                p.write(&mut r).unwrap();
                r.position()
            };

            let mut r = MqttBufReader::new(&buf[0..position]);
            assert_eq!(
                PacketFirstThreeProperty::read(&mut r),
                Err(mqtt_reader::MqttReaderError::MalformedPacket)
            );
        }
    }
}
