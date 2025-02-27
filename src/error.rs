use core::{
    fmt::{Display, Formatter},
    str::Utf8Error,
};

/// An error occurring while attempting to read/receive/decode an MQTT packet
/// Can occur at multiple levels:
/// 1. Reading data from a [Connection] - e.g. an IO error occurs in underlying data stream
/// 2. Reading data from a buffer - e.g. running out of data
/// 3. Decoding byte data as expected types, e.g. encountering an invalid boolean encoding (value not 0 or 1)
/// 4. Building a packet structure, e.g. there are too many properties or subscription request to fit in a fixed-length [heapless::Vec]
/// 5. Validating that packet matches MQTT specification, e.g. "reserved" bits of first header byte are not set to correct values
///
/// This does NOT include any errors found to be encoded in the packet itself - as long as these
/// errors are correctly decoded this doesn't represent a read error. Errors in the packet will
/// be handled at higher layers.
///
/// Note that these errors mostly map to [ReasonCode::MalformedPacket] when they result in an error
/// encoded in an MQTT packet, they are intended to provide a more granular description of the error
/// to assist in debugging and error handling. Other errors would not ever be represented in a packet,
/// since they represent errors like a failure to read from the network, etc.
///
/// Note that some errors map to a [ReasonCode] other than [ReasonCode::MalformedPacket], for example
/// [PacketReadError::UnexpectedPropertyIdentifier] may map to [ReasonCode::ProtocolError] in the case
/// where a reason string property is sent in a packet type that should not have such a property, see
/// [MQTT-3.1.2-29] in the specification.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PacketReadError {
    /// No more data was available at a point where the MQTT specification states more data
    /// must be present in the packet
    InsufficientData,

    /// Data contained bytes that could not be converted to a valid utf8 string
    InvalidUtf8,

    /// Data contained bytes that resulted in a utf8 string containing one or more null
    /// characters, which are not permitted in MQTT strings
    NullCharacterInString,

    /// While decoding a variable byte integer, the data did not match possible encodings
    /// (e.g. it contained more than 3 bytes with continuation bit set, indicating a total encoded
    /// length greater than 4 bytes)
    InvalidVariableByteIntegerEncoding,

    /// Data was expected to be of a known packet type, but the first header byte did not match this
    IncorrectPacketType,

    /// Data contained an unknown reason code
    UnknownReasonCode,

    /// Data contained a u8 value that was expected to be 0 (false) or 1 (true), but was some other value
    InvalidBooleanValue,

    /// Data contained a list of properties longer than the `PROPERTIES_N` parameter of a packet,
    /// and so overflowed a [heapless::Vec]
    TooManyProperties,

    /// Data contained an encoded [QualityOfService] value which was not a recognised value
    /// (Malformed Packet)
    InvalidQosValue,

    /// A Connect packet was decoded which did not contain the expected protocol name (MQTT) and version (5)
    /// This can be returned to clients, but it is also acceptable to simply close the network connection,
    /// see spec 3.1.2.1 and 3.1.2.2
    UnsupportedProtocolVersion,

    /// Data contained a list of subscription requests (or suback reason codes) longer than the `REQUEST_N`
    /// parameter of a packet, and so overflowed a [heapless::Vec]
    TooManyRequests,

    /// Data meant to encode a packet type had an invalid value. E.g. first header byte could not be
    /// decoded to a [PacketType], or contained invalid values for the "reserved" bits.
    InvalidPacketType,

    /// Failure to receive via connection
    ConnectionReceive,

    /// Packet was too large to place in provided buffer
    PacketTooLargeForBuffer,

    /// When decoding a property for a packet, encountered a value of the property identifier byte
    /// that was not expected in the given context. This may be an id that is completely unknown, or
    /// just one that is not expected for the type of packet being decoded.
    UnexpectedPropertyIdentifier,

    /// When decoding the "retain handling" subscription option, an invalid value not matching the
    /// specification was encountered
    InvalidRetainHandlingValue,

    /// When decoding connect flags of Connect packet, an invalid value was encountered (Malformed Packet)
    InvalidConnectFlags,

    /// When decoding a packet, the "remaining length" in the packet header was incorrect -
    /// the actual packet format indicates the packet is longer or shorter than header indicates
    IncorrectPacketLength,

    /// All Subscribe packets must have at least one subscription request [MQTT-3.8.3-2]
    SubscribeWithoutValidSubscriptionRequest,

    /// All Suback packets must have at least one reason code, since they are responding
    /// to a [Subscription] with at least one subscription request [MQTT-3.8.3-2]
    SubackWithoutValidReasonCode,

    /// All Unsubscribe packets must have at least one subscription request [MQTT-3.10.3-2]
    UnsubscribeWithoutValidSubscriptionRequest,

    /// All Unsuback packets must have at least one reason code, since they are responding
    /// to a [Unsubscribe] with at least one subscription request [MQTT-3.10.3-2]
    UnsubackWithoutValidReasonCode,

    // If a connect packet has no will specified, it must also have the will quality of service bits as 0 [MQTT-3.1.2-11]
    WillQosSpecifiedWithoutWill,

    // If a connect packet has no will specified, it must also have the will Retain bit as 0 [MQTT-3.1.2-13]
    WillRetainSpecifiedWithoutWill,

    // Subscription options u8 values must not have reserved bits set to non-zero [MQTT-3.8.3-5]
    SubscriptionOptionsReservedBitsNonZero,
}

impl From<Utf8Error> for PacketReadError {
    fn from(_e: Utf8Error) -> Self {
        Self::InvalidUtf8
    }
}

impl Display for PacketReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InsufficientData => write!(f, "InsufficientData"),
            Self::InvalidUtf8 => write!(f, "InvalidUtf8"),
            Self::NullCharacterInString => write!(f, "NullCharacterInString"),
            Self::InvalidVariableByteIntegerEncoding => {
                write!(f, "InvalidVariableByteIntegerEncoding")
            }
            Self::IncorrectPacketType => write!(f, "IncorrectPacketType"),
            Self::UnknownReasonCode => write!(f, "UnknownReasonCode"),
            Self::InvalidBooleanValue => write!(f, "InvalidBooleanValue"),
            Self::TooManyProperties => write!(f, "TooManyProperties"),
            Self::InvalidQosValue => write!(f, "InvalidQosValue"),
            Self::UnsupportedProtocolVersion => write!(f, "UnsupportedProtocolVersion"),
            Self::TooManyRequests => write!(f, "TooManyRequests"),
            Self::InvalidPacketType => write!(f, "InvalidPacketType"),
            Self::ConnectionReceive => write!(f, "ConnectionReceive"),
            Self::PacketTooLargeForBuffer => write!(f, "PacketTooLargeForBuffer"),
            Self::UnexpectedPropertyIdentifier => write!(f, "UnexpectedPropertyIdentifier"),
            Self::InvalidRetainHandlingValue => write!(f, "InvalidRetainHandlingValue"),
            Self::InvalidConnectFlags => write!(f, "InvalidConnectFlags"),
            Self::IncorrectPacketLength => write!(f, "IncorrectPacketLength"),
            Self::SubscribeWithoutValidSubscriptionRequest => {
                write!(f, "SubscribeWithoutValidSubscriptionRequest")
            }
            Self::SubackWithoutValidReasonCode => write!(f, "SubackWithoutValidReasonCode"),
            Self::UnsubscribeWithoutValidSubscriptionRequest => {
                write!(f, "UnsubscribeWithoutValidSubscriptionRequest")
            }
            Self::UnsubackWithoutValidReasonCode => write!(f, "UnsubackWithoutValidReasonCode"),
            Self::WillQosSpecifiedWithoutWill => write!(f, "WillQosSpecifiedWithoutWill"),
            Self::WillRetainSpecifiedWithoutWill => write!(f, "WillRetainSpecifiedWithoutWill"),
            Self::SubscriptionOptionsReservedBitsNonZero => {
                write!(f, "ReservedBitsSetInSubscriptionOptions")
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PacketWriteError {
    /// On attempt to put data that will not fit in buffer
    Overflow,

    /// On attempt to put a string containing a null character (which is not valid in an MQTT message)
    NullCharacterInString,

    /// On attempt to put a u32 value as a variable byte integer, where the value is too large to encode
    VariableByteIntegerTooLarge,

    /// On attempt to put binary data with too many bytes to encode
    DataTooLarge,

    /// On attempt to put a string where the encoded form is too many bytes to encode
    StringTooLarge,

    /// Failure to send via connection
    ConnectionSend,
}

impl Display for PacketWriteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Overflow => write!(f, "Overflow"),
            Self::NullCharacterInString => write!(f, "NullCharacterInString"),
            Self::VariableByteIntegerTooLarge => write!(f, "VariableByteIntegerTooLarge"),
            Self::DataTooLarge => write!(f, "DataTooLarge"),
            Self::StringTooLarge => write!(f, "StringTooLarge"),
            Self::ConnectionSend => write!(f, "ConnectionSend"),
        }
    }
}
