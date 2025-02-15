use core::str::Utf8Error;

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

    /// Data did not match the MQTT specification, and no more specific error was available
    MalformedPacket,

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
    InvalidQoSValue,

    /// A Connect packet was decoded which did not contain the expected protocol name (MQTT) and version (5)
    /// This can be returned to clients, but it is also acceptable to simply close the network connection,
    /// see spec 3.1.2.1 and 3.1.2.2
    UnsupportedProtocolVersion,

    /// Data contained a list of subscription requests longer than the `REQUEST_N` parameter of a packet,
    /// and so overflowed a [heapless::Vec]
    TooManyRequests,

    /// Data meant to encode a packet type had an invalid value. E.g. first header byte could not be
    /// decoded to a [PacketType], or contained invalid values for the "reserved" bits.
    InvalidPacketType,

    /// Failure to receive via connection
    ConnectionReceive,

    /// Packet was too large to place in provided buffer
    PacketTooLargeForBuffer,
}

impl From<Utf8Error> for PacketReadError {
    fn from(_e: Utf8Error) -> Self {
        Self::InvalidUtf8
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
