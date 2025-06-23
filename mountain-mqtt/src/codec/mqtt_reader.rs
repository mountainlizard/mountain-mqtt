use heapless::Vec;

use crate::{
    data::{
        reason_code::ReasonCode, string_pair::StringPair, subscription_options::SubscriptionOptions,
    },
    error::PacketReadError,
    packets::subscribe::SubscriptionRequest,
};

use super::read::Read;

pub type Result<A> = core::result::Result<A, PacketReadError>;

/// A reader providing access to an underlying buffer in the form of
/// MQTT v5 encoded data. Each `get` method retrieves data from the buffer,
/// advancing the position.
/// If a method fails (returns an [Err]), partial data may have been read
/// before the error was detected. Since an error always indicates invalid
/// data, there's no reason to allow recovering, and the reader should not
/// be used further.
pub trait MqttReader<'a>: Sized {
    /// Get the current position of the reader - this is 0 when reader is
    /// created, and increments by the number of bytes read on each successful
    /// `get` method call
    fn position(&self) -> usize;

    /// Get a slice with given length, as a view of this reader's buffer
    /// Advances the position by given length
    /// Can fail with [PacketReadError::InsufficientData]
    fn get_slice(&mut self, len: usize) -> Result<&'a [u8]>;

    // /// Make a view of this reader that is limited to returning at most
    // /// `remaining` bytes of data. If more than this is requested from the,
    // /// new reader, this will result in [PacketReadError::InsufficientData], even
    // /// if this reader itself still has more data.
    // /// This can be used to ensure that we respected the length of an embedded
    // /// piece of data, for example in an encoded properties list, where we
    // /// must stop reading data exactly at the limit specified in the encoded
    // /// property list.
    // ///
    // /// In order for this to work as expected, the underlying reader should not
    // /// be read from directly while the limited reader is in use - all reads should be
    // /// through the limited reader (this is not currently enforced).
    // ///
    // /// Note that limited readers can be nested, and the limits will be checked
    // /// from the deepest level of nesting upwards, if any limits are reached in any
    // /// nested reader this will error. In this case, only the deepest level of nesting
    // /// should be read from, and this will pass the request through each layer of nesting,
    // /// tracking and enforcing all limits.
    // ///
    // /// This design is intended to support structures used in MQTT packets - for example
    // /// the entire packet has a length as part of the fixed header, and this can be used
    // /// to apply a limit to read the rest of the whole packet, but then within the packet
    // /// there may be a properties list with a length, and this can be used to produce
    // /// another nested limit while reading the properties data. When this is complete, the
    // /// nested limited reader can be discarded and the "whole packet" reader used again.
    // ///
    // /// So for example calling limited with 70 bytes remaining could produce
    // /// a new reader `r1``, then calling `r1.limited` with 50 bytes remaining would
    // /// produce another reader `r2`. We can then read up to 50 bytes from `r2`.
    // /// If we read 50 bytes then discard `r2` we can continue reading from `r1`,
    // /// which will permit reading another 20 bytes (up to a total of 70 as specified).
    // /// Finally we can discard `r1` and resume reading from the original base
    // /// reader.
    // ///
    // fn limited(&'a mut self, remaining: usize) -> MqttLimitReader<'a, Self> {
    //     MqttLimitReader::new(self, remaining)
    // }

    /// Get the next byte of data as a bool, where
    /// false is encoded as 0, true as 1, and any other
    /// value gives [PacketReadError::InvalidBooleanValue]
    /// Advances the position by 1
    /// Can fail with [PacketReadError::InsufficientData]
    fn get_bool_zero_one(&mut self) -> Result<bool> {
        let value = self.get_u8()?;
        match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(PacketReadError::InvalidBooleanValue),
        }
    }

    /// Get the next byte of data as a u8
    /// Advances the position by 1
    /// Can fail with [PacketReadError::InsufficientData]
    fn get_u8(&mut self) -> Result<u8> {
        self.get_slice(1).map(|s| s[0])
    }

    /// Get the next two bytes of data as a u16, using big-endian conversion
    /// Advances the position by 2
    /// Can fail with [PacketReadError::InsufficientData]
    fn get_u16(&mut self) -> Result<u16> {
        let slice = self.get_slice(core::mem::size_of::<u16>())?;
        Ok(u16::from_be_bytes(slice.try_into().unwrap()))
    }

    /// Get the next four bytes of data as a u32, using big-endian conversion
    /// Advances the position by 4
    /// Can fail with [PacketReadError::InsufficientData]
    fn get_u32(&mut self) -> Result<u32> {
        let slice = self.get_slice(core::mem::size_of::<u32>())?;
        Ok(u32::from_be_bytes(slice.try_into().unwrap()))
    }

    /// Get the next 1-4 bytes of data as a u32, using "variable byte integer"
    /// conversion as specified by MQTT v5
    /// Advances the position by 1 to 4 bytes
    /// Can fail with [PacketReadError::InsufficientData] or [PacketReadError::InvalidVariableByteIntegerEncoding]
    fn get_variable_u32(&mut self) -> Result<u32> {
        let mut multiplier: u32 = 1;
        let mut value: u32 = 0;
        let mut encoded_len: usize = 0;

        loop {
            let encoded = self.get_u8()?;
            encoded_len += 1;

            value += (encoded & 127) as u32 * multiplier;
            multiplier *= 128;

            if encoded & 128 == 0 {
                break;
            } else if encoded_len > 3 {
                return Err(PacketReadError::InvalidVariableByteIntegerEncoding);
            }
        }

        Ok(value)
    }

    /// Get the next bytes of data as a str, using the next two bytes as
    /// a string length, then the following bytes as utf8 string data,
    /// as specified by MQTT v5
    /// Advances the position by 2 or more bytes on success.
    /// If the decoded string contains any null characters, will fail with
    /// [PacketReadError::NullCharacterInString].
    /// If the buffer data contains invalid utf8 data, will fail with
    /// [PacketReadError::InvalidUtf8].
    /// Can fail with [PacketReadError::InsufficientData],
    fn get_str(&mut self) -> Result<&'a str> {
        let len = self.get_u16()?;
        let slice = self.get_slice(len as usize)?;
        let s = core::str::from_utf8(slice)?;
        if s.contains("\0") {
            Err(PacketReadError::NullCharacterInString)
        } else {
            Ok(s)
        }
    }

    /// Get the next bytes of data as a [StringPair], by attempting to read
    /// a name as a string, then a value as a string, as specified by MQTT v5
    /// Advances the position by 4 or more bytes on success.
    /// If either decoded string contains any null characters, will fail with
    /// [PacketReadError::NullCharacterInString].
    /// If the buffer data contains invalid utf8 data for either strnig, will fail with
    /// [PacketReadError::InvalidUtf8].
    /// Can fail with [PacketReadError::InsufficientData],
    fn get_string_pair(&mut self) -> Result<StringPair<'a>> {
        let name = self.get_str()?;
        let value = self.get_str()?;
        Ok(StringPair::new(name, value))
    }

    /// Get the next bytes of data as a delimited binary data item,
    /// using the next two bytes as a data length, then the following
    /// bytes as the data itself, as specified by MQTT v5.
    /// Advances the position by 2 or more bytes on success.
    /// Can fail with [PacketReadError::InsufficientData],
    fn get_binary_data(&mut self) -> Result<&'a [u8]> {
        let len = self.get_u16()?;
        self.get_slice(len as usize)
    }

    fn get_reason_code(&mut self) -> Result<ReasonCode> {
        let value = self.get_u8()?;
        match value {
            0 => Ok(ReasonCode::Success),
            1 => Ok(ReasonCode::GrantedQos1),
            2 => Ok(ReasonCode::GrantedQos2),
            4 => Ok(ReasonCode::DisconnectWithWillMessage),
            16 => Ok(ReasonCode::NoMatchingSubscribers),
            17 => Ok(ReasonCode::NoSubscriptionExisted),
            24 => Ok(ReasonCode::ContinueAuthentication),
            25 => Ok(ReasonCode::ReAuthenticate),
            128 => Ok(ReasonCode::UnspecifiedError),
            129 => Ok(ReasonCode::MalformedPacket),
            130 => Ok(ReasonCode::ProtocolError),
            131 => Ok(ReasonCode::ImplementationSpecificError),
            132 => Ok(ReasonCode::UnsupportedProtocolVersion),
            133 => Ok(ReasonCode::ClientIdentifierNotValid),
            134 => Ok(ReasonCode::BadUserNameOrPassword),
            135 => Ok(ReasonCode::NotAuthorized),
            136 => Ok(ReasonCode::ServerUnavailable),
            137 => Ok(ReasonCode::ServerBusy),
            138 => Ok(ReasonCode::Banned),
            139 => Ok(ReasonCode::ServerShuttingDown),
            140 => Ok(ReasonCode::BadAuthenticationMethod),
            141 => Ok(ReasonCode::KeepAliveTimeout),
            142 => Ok(ReasonCode::SessionTakenOver),
            143 => Ok(ReasonCode::TopicFilterInvalid),
            144 => Ok(ReasonCode::TopicNameInvalid),
            145 => Ok(ReasonCode::PacketIdentifierInUse),
            146 => Ok(ReasonCode::PacketIdentifierNotFound),
            147 => Ok(ReasonCode::ReceiveMaximumExceeded),
            148 => Ok(ReasonCode::TopicAliasInvalid),
            149 => Ok(ReasonCode::PacketTooLarge),
            150 => Ok(ReasonCode::MessageRateTooHigh),
            151 => Ok(ReasonCode::QuotaExceeded),
            152 => Ok(ReasonCode::AdministrativeAction),
            153 => Ok(ReasonCode::PayloadFormatInvalid),
            154 => Ok(ReasonCode::RetainNotSupported),
            155 => Ok(ReasonCode::QosNotSupported),
            156 => Ok(ReasonCode::UseAnotherServer),
            157 => Ok(ReasonCode::ServerMoved),
            158 => Ok(ReasonCode::SharedSubscriptionsNotSupported),
            159 => Ok(ReasonCode::ConnectionRateExceeded),
            160 => Ok(ReasonCode::MaximumConnectTime),
            161 => Ok(ReasonCode::SubscriptionIdentifiersNotSupported),
            162 => Ok(ReasonCode::WildcardSubscriptionsNotSupported),
            _ => Err(PacketReadError::UnknownReasonCode),
        }
    }

    fn get_property_list<T: Read<'a>, const N: usize>(
        &mut self,
        vec: &mut Vec<T, N>,
    ) -> Result<()> {
        let properties_len = self.get_variable_u32()? as usize;
        let properties_end = self.position() + properties_len;

        while self.position() < properties_end {
            let item = T::read(self)?;
            vec.push(item)
                .map_err(|_e| PacketReadError::TooManyProperties)?;
        }

        Ok(())
    }

    fn get_subscription_options(&mut self) -> Result<SubscriptionOptions> {
        let encoded = self.get_u8()?;
        encoded.try_into()
    }

    fn get_subscription_request(&mut self) -> Result<SubscriptionRequest<'a>> {
        let topic_name = self.get_str()?;
        let options = self.get_subscription_options()?;
        Ok(SubscriptionRequest {
            topic_name,
            options,
        })
    }

    fn get<T: Read<'a>>(&mut self) -> Result<T> {
        T::read(self)
    }
}

pub struct MqttBufReader<'a> {
    buf: &'a [u8],
    position: usize,
}

impl<'a> MqttBufReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, position: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.position
    }
}

impl<'a> MqttReader<'a> for MqttBufReader<'a> {
    fn position(&self) -> usize {
        self.position
    }

    fn get_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.position + len;
        if end > self.buf.len() {
            Err(PacketReadError::InsufficientData)
        } else {
            let slice = &self.buf[self.position..end];
            self.position = end;
            Ok(slice)
        }
    }
}

// pub struct MqttLimitReader<'a, R>
// where
//     R: MqttReader<'a>,
// {
//     reader: &'a mut R,
//     remaining: usize,
// }

// impl<'a, R> MqttLimitReader<'a, R>
// where
//     R: MqttReader<'a>,
// {
//     pub fn new(reader: &'a mut R, remaining: usize) -> MqttLimitReader<'a, R> {
//         MqttLimitReader { reader, remaining }
//     }

//     pub fn remaining(&self) -> usize {
//         self.remaining
//     }
// }

// impl<'a, R> MqttReader<'a> for MqttLimitReader<'a, R>
// where
//     R: MqttReader<'a>,
// {
//     fn get_slice(&mut self, len: usize) -> Result<&'a [u8]> {
//         if len > self.remaining {
//             Err(PacketReadError::InsufficientData)
//         } else {
//             self.remaining -= len;
//             self.reader.get_slice(len)
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    // Note - tests for `get_variable_u32_delimited_vec` are in property module
    // Note - tests for `get_subscription_options` are in subscribe module

    #[test]
    fn mqtt_buf_reader_can_get_slices() -> Result<()> {
        let buf = [0, 1, 2, 3];
        let mut r = MqttBufReader::new(&buf);

        assert_eq!(4, r.remaining());

        let slice_0 = r.get_slice(1)?;
        let slice_1_2 = r.get_slice(2)?;
        let slice_3 = r.get_slice(1)?;

        assert_eq!(slice_0, &[0u8]);
        assert_eq!(slice_1_2, &[1u8, 2u8]);
        assert_eq!(slice_3, &[3u8]);

        assert_eq!(0, r.remaining());

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_errors_on_overrun() -> Result<()> {
        let buf = [0, 1, 2, 3];
        let mut r = MqttBufReader::new(&buf);

        r.get_slice(1)?;
        r.get_slice(2)?;
        r.get_slice(1)?;

        assert_eq!(0, r.remaining());

        assert_eq!(r.get_slice(1), Err(PacketReadError::InsufficientData));
        assert_eq!(r.get_slice(2), Err(PacketReadError::InsufficientData));

        // Can still get an empty slice
        let slice_empty = r.get_slice(0)?;
        assert_eq!(slice_empty, &[]);

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_bool_zero_one() -> Result<()> {
        let buf = [0, 1, 0, 1];
        let mut r = MqttBufReader::new(&buf);

        for value in buf.iter() {
            let expected = *value == 1;
            assert_eq!(expected, r.get_bool_zero_one()?);
        }

        // We've consumed all data
        assert_eq!(0, r.remaining());
        assert_eq!(r.get_slice(1), Err(PacketReadError::InsufficientData));

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_fails_on_bool_zero_one_invalid_values() -> Result<()> {
        for value in 2u8..255 {
            let buf = [0, 1, value];
            let mut r = MqttBufReader::new(&buf);

            assert!(!r.get_bool_zero_one()?);
            assert!(r.get_bool_zero_one()?);
            assert_eq!(
                Err(PacketReadError::InvalidBooleanValue),
                r.get_bool_zero_one()
            );
        }
        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_u8() -> Result<()> {
        let buf = [0, 1, 2, 3, 4, 128, 255];
        let mut r = MqttBufReader::new(&buf);

        for expected in buf.iter() {
            assert_eq!(*expected, r.get_u8()?);
        }

        // We've consumed all data
        assert_eq!(0, r.remaining());
        assert_eq!(r.get_slice(1), Err(PacketReadError::InsufficientData));

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_u16() -> Result<()> {
        let buf = [0, 1, 2, 3, 4, 128, 255];
        let mut r = MqttBufReader::new(&buf);

        assert_eq!(1, r.get_u16()?);
        assert_eq!((2 << 8) + 3, r.get_u16()?);
        assert_eq!((4 << 8) + 128, r.get_u16()?);

        // Single remaining byte can't be read as a u16
        assert_eq!(1, r.remaining());
        assert_eq!(r.get_u16(), Err(PacketReadError::InsufficientData));

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_u32() -> Result<()> {
        let buf = [0, 1, 2, 3, 4, 128, 255, 42, 1, 2, 3];
        let mut r = MqttBufReader::new(&buf);

        assert_eq!((1 << 16) + (2 << 8) + 3, r.get_u32()?);
        assert_eq!((4 << 24) + (128 << 16) + (255 << 8) + 42, r.get_u32()?);

        // Three remaining bytes can't be read as a u32
        assert_eq!(3, r.remaining());
        assert_eq!(r.get_u32(), Err(PacketReadError::InsufficientData));

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_variable_u32() -> Result<()> {
        let test_cases: &[(u32, &[u8])] = &[
            // Examples taken from mqtt v5 specification, testing the ends of the ranges
            // of values stored in 1, 2, 3 and 4 bytes
            (0, &[0x00]),
            (127, &[0x7F]),
            (128, &[0x80, 0x01]),
            (16_383, &[0xFF, 0x7F]),
            (16_384, &[0x80, 0x80, 0x01]),
            (2_097_151, &[0xFF, 0xFF, 0x7F]),
            (2_097_152, &[0x80, 0x80, 0x80, 0x01]),
            (268_435_455, &[0xFF, 0xFF, 0xFF, 0x7F]),
            // Additional
            (1, &[1]),
            (42, &[42]),
            (130, &[0x82, 0x01]),
            (384, &[0x80, 0x03]),
            (16_382, &[0xFE, 0x7F]),
            (16_385, &[0x81, 0x80, 0x01]),
            (16_386, &[0x82, 0x80, 0x01]),
            (32_768, &[0x80, 0x80, 0x02]),
        ];

        for (expected, data) in test_cases.iter() {
            let mut r = MqttBufReader::new(data);

            assert_eq!(*expected, r.get_variable_u32()?);
            assert_eq!(0, r.remaining());
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_errors_on_invalid_variable_u32() -> Result<()> {
        // 4 0x80's in a row indicates that there is still more data after
        // 4 bytes, which is invalid and should produce an error
        let buf = [0x80, 0x80, 0x80, 0x80];
        let mut r = MqttBufReader::new(&buf);

        assert_eq!(
            r.get_variable_u32(),
            Err(PacketReadError::InvalidVariableByteIntegerEncoding)
        );

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_str() -> Result<()> {
        let test_cases: &[(&str, &[u8])] = &[
            // Example taken from mqtt v5 specification
            ("A\u{2A6D4}", &[0x00, 0x05, 0x41, 0xF0, 0xAA, 0x9B, 0x94]),
            // Specific requirement from mqtt v5 specification:
            // A UTF-8 encoded sequence 0xEF 0xBB 0xBF is always interpreted
            // as U+FEFF ("ZERO WIDTH NO-BREAK SPACE") wherever it appears
            // in a string and MUST NOT be skipped over or stripped off
            // by a packet receiver
            ("\u{FEFF}", &[0x00, 0x03, 0xEF, 0xBB, 0xBF]),
            ("A\u{FEFF}", &[0x00, 0x04, 0x41, 0xEF, 0xBB, 0xBF]),
            ("\u{FEFF}A", &[0x00, 0x04, 0xEF, 0xBB, 0xBF, 0x41]),
            ("A\u{FEFF}A", &[0x00, 0x05, 0x41, 0xEF, 0xBB, 0xBF, 0x41]),
            // Additional
            ("", &[0x00, 0x00]),
            ("A", &[0x00, 0x01, 0x41]),
            ("ABCDE", &[0x00, 0x05, 0x41, 0x42, 0x43, 0x44, 0x45]),
            ("😼", &[0x00, 0x04, 0xF0, 0x9F, 0x98, 0xBC]),
        ];

        for (expected, data) in test_cases.iter() {
            let mut r = MqttBufReader::new(data);

            assert_eq!(*expected, r.get_str()?);
            assert_eq!(0, r.remaining());
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_errors_on_invalid_str() -> Result<()> {
        let test_cases: &[(&[u8], PacketReadError)] = &[
            // Character data MUST NOT include encodings of code points between U+D800 and U+DFFF
            // U+D800
            (
                &[0x00, 0x03, 0xED, 0xA0, 0x80],
                PacketReadError::InvalidUtf8,
            ),
            // U+D900
            (
                &[0x00, 0x03, 0xED, 0xA4, 0x80],
                PacketReadError::InvalidUtf8,
            ),
            // U+DFFF
            (
                &[0x00, 0x03, 0xED, 0xBF, 0xBF],
                PacketReadError::InvalidUtf8,
            ),
            // A UTF-8 Encoded String MUST NOT include an encoding of the null character U+0000.
            (&[0x00, 0x01, 0x00], PacketReadError::NullCharacterInString),
        ];

        for (data, err) in test_cases.iter() {
            let mut r = MqttBufReader::new(data);

            assert_eq!(r.get_str(), Err(*err));
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_errors_on_insufficient_data_for_string() -> Result<()> {
        let buf = [0x00, 0x03, 0x80, 0x80];
        let mut r = MqttBufReader::new(&buf);

        assert_eq!(r.get_str(), Err(PacketReadError::InsufficientData));

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_binary_data() -> Result<()> {
        let test_cases: &[(&[u8], &[u8])] = &[
            (&[], &[0x00, 0x00]),
            (&[0x41], &[0x00, 0x01, 0x41]),
            (
                &[0x41, 0xF0, 0xAA, 0x9B, 0x94],
                &[0x00, 0x05, 0x41, 0xF0, 0xAA, 0x9B, 0x94],
            ),
            (&[0xEF, 0xBB, 0xBF], &[0x00, 0x03, 0xEF, 0xBB, 0xBF]),
            (
                &[0x41, 0xEF, 0xBB, 0xBF],
                &[0x00, 0x04, 0x41, 0xEF, 0xBB, 0xBF],
            ),
            (
                &[0xEF, 0xBB, 0xBF, 0x41],
                &[0x00, 0x04, 0xEF, 0xBB, 0xBF, 0x41],
            ),
            (
                &[0x41, 0xEF, 0xBB, 0xBF, 0x41],
                &[0x00, 0x05, 0x41, 0xEF, 0xBB, 0xBF, 0x41],
            ),
        ];

        for (expected, data) in test_cases.iter() {
            let mut r = MqttBufReader::new(data);

            assert_eq!(*expected, r.get_binary_data()?);
            assert_eq!(0, r.remaining());
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_errors_on_insufficient_data_for_binary_data() -> Result<()> {
        let buf = [0x00, 0x03, 0x80, 0x80];
        let mut r = MqttBufReader::new(&buf);

        assert_eq!(r.get_binary_data(), Err(PacketReadError::InsufficientData));

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_string_pairs() -> Result<()> {
        let test_cases: &[(&str, &[u8])] = &[
            // Example taken from mqtt v5 specification
            ("A\u{2A6D4}", &[0x00, 0x05, 0x41, 0xF0, 0xAA, 0x9B, 0x94]),
            // Specific requirement from mqtt v5 specification:
            // A UTF-8 encoded sequence 0xEF 0xBB 0xBF is always interpreted
            // as U+FEFF ("ZERO WIDTH NO-BREAK SPACE") wherever it appears
            // in a string and MUST NOT be skipped over or stripped off
            // by a packet receiver
            ("\u{FEFF}", &[0x00, 0x03, 0xEF, 0xBB, 0xBF]),
            ("A\u{FEFF}", &[0x00, 0x04, 0x41, 0xEF, 0xBB, 0xBF]),
            ("\u{FEFF}A", &[0x00, 0x04, 0xEF, 0xBB, 0xBF, 0x41]),
            ("A\u{FEFF}A", &[0x00, 0x05, 0x41, 0xEF, 0xBB, 0xBF, 0x41]),
            // Additional
            ("", &[0x00, 0x00]),
            ("A", &[0x00, 0x01, 0x41]),
            ("ABCDE", &[0x00, 0x05, 0x41, 0x42, 0x43, 0x44, 0x45]),
            ("😼", &[0x00, 0x04, 0xF0, 0x9F, 0x98, 0xBC]),
        ];

        let mut encoded_string_pair = [0u8; 30];

        // Test each pair of example strings as a name and value of a string pair
        for (name, encoded_name) in test_cases.iter() {
            for (value, encoded_value) in test_cases.iter() {
                // Assemble hand-encoded string pair data
                let total_len = encoded_name.len() + encoded_value.len();
                encoded_string_pair[0..encoded_name.len()].copy_from_slice(encoded_name);
                encoded_string_pair[encoded_name.len()..total_len].copy_from_slice(encoded_value);

                // Read back hand-encoded data as string pair, and check it matches expected name and value
                let mut r = MqttBufReader::new(&encoded_string_pair[0..total_len]);
                let string_pair = r.get_string_pair()?;
                assert_eq!(*name, string_pair.name());
                assert_eq!(*value, string_pair.value());
                assert_eq!(0, r.remaining());
            }
        }
        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_can_get_reason_code() -> Result<()> {
        // Test a selection of codes
        let codes = [
            ReasonCode::AdministrativeAction,
            ReasonCode::Banned,
            ReasonCode::GrantedQos1,
            ReasonCode::PayloadFormatInvalid,
            ReasonCode::Success,
            ReasonCode::UnspecifiedError,
            ReasonCode::WildcardSubscriptionsNotSupported,
        ];

        for code in codes.iter() {
            let buf = [*code as u8];
            let mut r = MqttBufReader::new(&buf);
            let read_code = r.get_reason_code()?;
            assert_eq!(read_code, *code);
            assert_eq!(1, r.position());
            assert_eq!(0, r.remaining());
        }

        Ok(())
    }
}
