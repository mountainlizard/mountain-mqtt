use crate::data::{
    mqtt_reader::{self, MqttReader},
    mqtt_writer::{self, MqttWriter},
    read::Read,
    write::Write,
};

use super::string_pair::StringPair;

pub trait Property {
    const IDENTIFIER: u32;
}

#[macro_export]
macro_rules! property_u8 {
    ( $n:ident, $c:literal ) => {
        pub struct $n(u8);

        impl Write for $n {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_u8(self.0)
            }
        }

        impl<'a> Read<'a> for $n {
            fn read<R: MqttReader<'a>>(&self, reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                Ok($n(reader.get_u8()?))
            }
        }

        impl Property for $n {
            const IDENTIFIER: u32 = $c;
        }
    };
}

#[macro_export]
macro_rules! property_u16 {
    ( $n:ident, $c:literal ) => {
        pub struct $n(u16);

        impl Write for $n {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_u16(self.0)
            }
        }

        impl<'a> Read<'a> for $n {
            fn read<R: MqttReader<'a>>(&self, reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                Ok($n(reader.get_u16()?))
            }
        }

        impl Property for $n {
            const IDENTIFIER: u32 = $c;
        }
    };
}

#[macro_export]
macro_rules! property_u32 {
    ( $n:ident, $c:literal ) => {
        pub struct $n(u32);

        impl Write for $n {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_u32(self.0)
            }
        }

        impl<'a> Read<'a> for $n {
            fn read<R: MqttReader<'a>>(&self, reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                Ok($n(reader.get_u32()?))
            }
        }

        impl Property for $n {
            const IDENTIFIER: u32 = $c;
        }
    };
}

#[macro_export]
macro_rules! property_variable_u32 {
    ( $n:ident, $c:literal ) => {
        pub struct $n(u32);

        impl Write for $n {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_variable_u32(self.0)
            }
        }

        impl<'a> Read<'a> for $n {
            fn read<R: MqttReader<'a>>(&self, reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                Ok($n(reader.get_variable_u32()?))
            }
        }

        impl Property for $n {
            const IDENTIFIER: u32 = $c;
        }
    };
}

#[macro_export]
macro_rules! property_str {
    ( $n:ident, $c:literal ) => {
        pub struct $n<'a>(&'a str);

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_str(self.0)
            }
        }

        impl<'a> Read<'a> for $n<'a> {
            fn read<R: MqttReader<'a>>(&self, reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                Ok($n(reader.get_str()?))
            }
        }

        impl Property for $n<'_> {
            const IDENTIFIER: u32 = $c;
        }
    };
}

#[macro_export]
macro_rules! property_string_pair {
    ( $n:ident, $c:literal ) => {
        pub struct $n<'a>(StringPair<'a>);

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_string_pair(&self.0)
            }
        }

        impl<'a> Read<'a> for $n<'a> {
            fn read<R: MqttReader<'a>>(&self, reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                Ok($n(reader.get_string_pair()?))
            }
        }

        impl Property for $n<'_> {
            const IDENTIFIER: u32 = $c;
        }
    };
}

#[macro_export]
macro_rules! property_binary_data {
    ( $n:ident, $c:literal ) => {
        pub struct $n<'a>(&'a [u8]);

        impl Write for $n<'_> {
            fn write<'a, W: MqttWriter<'a>>(&self, writer: &mut W) -> mqtt_writer::Result<()> {
                writer.put_binary_data(self.0)
            }
        }

        impl<'a> Read<'a> for $n<'a> {
            fn read<R: MqttReader<'a>>(&self, reader: &mut R) -> mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                Ok($n(reader.get_binary_data()?))
            }
        }

        impl Property for $n<'_> {
            const IDENTIFIER: u32 = $c;
        }
    };
}

// Macros define the struct for each property type, and implement Read and Write for it

property_u8!(PayloadFormatIndicator, 0x01);
property_u32!(MessageExpiryInterval, 0x02);
property_str!(ContentType, 0x03);
property_str!(ResponseTopic, 0x08);
property_binary_data!(CorrelationData, 0x09);
property_variable_u32!(SubscriptionIdentifier, 0x0B);
property_u32!(SessionExpiryInterval, 0x11);
property_str!(AssignedClientIdentifier, 0x12);
property_u16!(ServerKeepAlive, 0x13);
property_str!(AuthenticationMethod, 0x15);
property_binary_data!(AuthenticationData, 0x16);
property_u8!(RequestProblemInformation, 0x17);
property_u32!(WillDelayInterval, 0x18);
property_u8!(RequestResponseInformation, 0x19);
property_str!(ResponseInformation, 0x1A);
property_str!(ServerReference, 0x1C);
property_str!(ReasonString, 0x1F);
property_u16!(ReceiveMaximum, 0x21);
property_u16!(TopicAliasMaximum, 0x22);
property_u16!(TopicAlias, 0x23);
property_u8!(MaximumQoS, 0x24);
property_u8!(RetainAvailable, 0x25);
property_string_pair!(UserProperty, 0x26);
property_u32!(MaximumPacketSize, 0x27);
property_u8!(WildcardSubscriptionAvailable, 0x28);
property_u8!(SubscriptionIdentifierAvailable, 0x29);
property_u8!(SharedSubscriptionAvailable, 0x2A);

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
