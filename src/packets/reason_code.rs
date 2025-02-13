use crate::data::{mqtt_reader::MqttReaderError, read::Read, write::Write};

#[macro_export]
macro_rules! packet_reason_codes {
    ( $n:ident, [ $( $c:ident ),+ ] ) => {
        #[derive(Debug, PartialEq, Clone, Copy)]
        #[repr(u8)]
        pub enum $n{
            $(
                $c = ReasonCode::$c as u8,
            )*
        }

        impl From<$n> for ReasonCode {
            fn from(value: $n) -> Self {
                match value {
                    $(
                        $n::$c => ReasonCode::$c,
                    )*
                }
            }
        }

        impl TryFrom<u8> for $n {
            type Error = MqttReaderError;

            fn try_from(value: u8) -> Result<Self, Self::Error> {
                $(
                    if value == ReasonCode::$c as u8 {
                        Ok(Self::$c)
                    } else
                )*
                {
                    Err(MqttReaderError::UnknownReasonCode)
                }
            }
        }

        impl TryFrom<ReasonCode> for $n {
            type Error = MqttReaderError;

            fn try_from(value: ReasonCode) -> Result<Self, Self::Error> {
                match value {
                    $(
                        ReasonCode::$c => Ok(Self::$c),
                    )*

                    _ => Err(MqttReaderError::UnknownReasonCode),
                }
            }
        }

        impl $n {
            pub fn is_error(&self) -> bool {
                (*self as u8) > 128
            }
        }

        impl<'a> Read<'a> for $n {
            fn read<R: $crate::data::mqtt_reader::MqttReader<'a>>(
                reader: &mut R,
            ) -> $crate::data::mqtt_reader::Result<Self>
            where
                Self: Sized,
            {
                let encoded = reader.get_u8()?;
                encoded.try_into()
            }
        }

        impl Write for $n {
            fn write<'a, W: $crate::data::mqtt_writer::MqttWriter<'a>>(
                &self,
                writer: &mut W,
            ) -> $crate::data::mqtt_writer::Result<()> {
                writer.put_u8(*self as u8)
            }
        }

    };
}

/// A Reason Code is a one byte unsigned value that indicates the result of an operation. Reason Codes less
/// than 128 indicate successful completion of an operation. The normal Reason Code for success is 0.
/// Reason Code values of 128 or greater indicate failure.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ReasonCode {
    Success = 0,
    GrantedQoS1 = 1,
    GrantedQoS2 = 2,
    DisconnectWithWillMessage = 4,
    NoMatchingSubscribers = 16,
    NoSubscriptionExisted = 17,
    ContinueAuthentication = 24,
    ReAuthenticate = 25,
    UnspecifiedError = 128,
    MalformedPacket = 129,
    ProtocolError = 130,
    ImplementationSpecificError = 131,
    UnsupportedProtocolVersion = 132,
    ClientIdentifierNotValid = 133,
    BadUserNameOrPassword = 134,
    NotAuthorized = 135,
    ServerUnavailable = 136,
    ServerBusy = 137,
    Banned = 138,
    ServerShuttingDown = 139,
    BadAuthenticationMethod = 140,
    KeepAliveTimeout = 141,
    SessionTakenOver = 142,
    TopicFilterInvalid = 143,
    TopicNameInvalid = 144,
    PacketIdentifierInUse = 145,
    PacketIdentifierNotFound = 146,
    ReceiveMaximumExceeded = 147,
    TopicAliasInvalid = 148,
    PacketTooLarge = 149,
    MessageRateTooHigh = 150,
    QuotaExceeded = 151,
    AdministrativeAction = 152,
    PayloadFormatInvalid = 153,
    RetainNotSupported = 154,
    QoSNotSupported = 155,
    UseAnotherServer = 156,
    ServerMoved = 157,
    SharedSubscriptionsNotSupported = 158,
    ConnectionRateExceeded = 159,
    MaximumConnectTime = 160,
    SubscriptionIdentifiersNotSupported = 161,
    WildcardSubscriptionsNotSupported = 162,
}

impl ReasonCode {
    pub fn is_error(&self) -> bool {
        (*self as u8) > 128
    }
}

packet_reason_codes!(
    ConnectReasonCode,
    [
        Success,
        UnspecifiedError,
        MalformedPacket,
        ProtocolError,
        ImplementationSpecificError,
        UnsupportedProtocolVersion,
        ClientIdentifierNotValid,
        BadUserNameOrPassword,
        NotAuthorized,
        ServerUnavailable,
        ServerBusy,
        Banned,
        BadAuthenticationMethod,
        TopicNameInvalid,
        PacketTooLarge,
        QuotaExceeded,
        PayloadFormatInvalid,
        RetainNotSupported,
        QoSNotSupported,
        UseAnotherServer,
        ServerMoved,
        ConnectionRateExceeded
    ]
);

packet_reason_codes!(
    DisconnectReasonCode,
    [
        Success, //Normal disconnection in spec
        DisconnectWithWillMessage,
        UnspecifiedError,
        MalformedPacket,
        ProtocolError,
        ImplementationSpecificError,
        NotAuthorized,
        ServerBusy,
        ServerShuttingDown,
        KeepAliveTimeout,
        SessionTakenOver,
        TopicFilterInvalid,
        TopicNameInvalid,
        ReceiveMaximumExceeded,
        TopicAliasInvalid,
        PacketTooLarge,
        MessageRateTooHigh,
        QuotaExceeded,
        AdministrativeAction,
        PayloadFormatInvalid,
        RetainNotSupported,
        QoSNotSupported,
        UseAnotherServer,
        ServerMoved,
        SharedSubscriptionsNotSupported,
        ConnectionRateExceeded,
        MaximumConnectTime,
        SubscriptionIdentifiersNotSupported,
        WildcardSubscriptionsNotSupported
    ]
);

packet_reason_codes!(
    PublishReasonCode,
    [
        Success,
        NoMatchingSubscribers,
        UnspecifiedError,
        ImplementationSpecificError,
        NotAuthorized,
        TopicNameInvalid,
        PacketIdentifierInUse,
        QuotaExceeded,
        PayloadFormatInvalid
    ]
);

packet_reason_codes!(PubrelReasonCode, [Success, PacketIdentifierNotFound]);

packet_reason_codes!(
    SubscriptionReasonCode,
    [
        Success,
        GrantedQoS1,
        GrantedQoS2,
        UnspecifiedError,
        ImplementationSpecificError,
        NotAuthorized,
        TopicFilterInvalid,
        PacketIdentifierInUse,
        QuotaExceeded,
        SharedSubscriptionsNotSupported,
        SubscriptionIdentifiersNotSupported,
        WildcardSubscriptionsNotSupported
    ]
);

packet_reason_codes!(
    UnsubscriptionReasonCode,
    [
        Success,
        NoSubscriptionExisted,
        UnspecifiedError,
        ImplementationSpecificError,
        NotAuthorized,
        TopicFilterInvalid,
        PacketIdentifierInUse
    ]
);

#[cfg(test)]
mod tests {
    use crate::data::{
        mqtt_reader::{MqttBufReader, MqttReader},
        mqtt_writer::{MqttBufWriter, MqttWriter},
    };

    use super::*;

    packet_reason_codes!(ExampleReasonCode, [UnspecifiedError, MalformedPacket]);

    const DATA: [(ExampleReasonCode, u8); 2] = [
        (
            ExampleReasonCode::UnspecifiedError,
            ReasonCode::UnspecifiedError as u8,
        ),
        (
            ExampleReasonCode::MalformedPacket,
            ReasonCode::MalformedPacket as u8,
        ),
    ];

    #[test]
    fn all_expected_codes_exist_with_expected_u8_value() {
        for (code, value) in DATA.iter() {
            assert_eq!(*code as u8, *value);
        }
    }

    #[test]
    fn can_write_codes() {
        for (code, value) in DATA.iter() {
            let mut buf = [0u8];
            {
                let mut r = MqttBufWriter::new(&mut buf);
                r.put(code).unwrap();
                assert_eq!(r.position(), 1);
                assert_eq!(r.remaining(), 0);
            }
            assert_eq!(buf, [*value]);
        }
    }

    #[test]
    fn can_read_codes() {
        for (code, value) in DATA.iter() {
            let buf = [*value];
            let mut r = MqttBufReader::new(&buf);
            let read_code: ExampleReasonCode = r.get().unwrap();
            assert_eq!(r.position(), 1);
            assert_eq!(r.remaining(), 0);
            assert_eq!(&read_code, code);
        }
    }

    #[test]
    fn can_try_to_get_packet_reason_code_from_reason_code() {
        assert_eq!(
            ExampleReasonCode::try_from(ReasonCode::UnspecifiedError),
            Ok(ExampleReasonCode::UnspecifiedError)
        );
        assert_eq!(
            ExampleReasonCode::try_from(ReasonCode::MalformedPacket),
            Ok(ExampleReasonCode::MalformedPacket)
        );
        assert_eq!(
            ExampleReasonCode::try_from(ReasonCode::AdministrativeAction),
            Err(MqttReaderError::UnknownReasonCode)
        );
    }

    #[test]
    fn can_try_to_get_packet_reason_code_from_u8() {
        assert_eq!(
            ExampleReasonCode::try_from(ReasonCode::UnspecifiedError as u8),
            Ok(ExampleReasonCode::UnspecifiedError)
        );
        assert_eq!(
            ExampleReasonCode::try_from(ReasonCode::MalformedPacket as u8),
            Ok(ExampleReasonCode::MalformedPacket)
        );
        assert_eq!(
            ExampleReasonCode::try_from(ReasonCode::AdministrativeAction as u8),
            Err(MqttReaderError::UnknownReasonCode)
        );
    }

    #[test]
    fn can_get_reason_code_from_packet_reason_code() {
        assert_eq!(
            ReasonCode::from(ExampleReasonCode::UnspecifiedError),
            ReasonCode::UnspecifiedError
        );
        assert_eq!(
            ReasonCode::from(ExampleReasonCode::MalformedPacket),
            ReasonCode::MalformedPacket
        );
    }
}
