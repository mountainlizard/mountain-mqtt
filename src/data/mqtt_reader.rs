use core::str::Utf8Error;

use crate::packets::string_pair::StringPair;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MqttReaderError {
    InsufficientData,
    InvalidUtf8,
    NullCharacterInString,
    InvalidVariableByteIntegerEncoding,
    MalformedPacket,
}

impl From<Utf8Error> for MqttReaderError {
    fn from(_e: Utf8Error) -> Self {
        Self::InvalidUtf8
    }
}

pub type Result<A> = core::result::Result<A, MqttReaderError>;

/// A reader providing access to an underlying buffer in the form of
/// MQTT v5 encoded data. Each `get` method retrieves data from the buffer,
/// advancing the position.
/// If a method fails (returns an [Err]), partial data may have been read
/// before the error was detected. Since an error always indicates invalid
/// data, there's no reason to allow recovering, and the reader should not
/// be used further.
pub trait MqttReader<'a> {
    /// Get a slice with given length, as a view of this reader's buffer
    /// Advances the position by given length
    /// Can fail with [MqttReaderError::InsufficientData]
    fn get_slice(&mut self, len: usize) -> Result<&'a [u8]>;

    /// Get the next byte of data as a u8
    /// Advances the position by 1
    /// Can fail with [MqttReaderError::InsufficientData]
    fn get_u8(&mut self) -> Result<u8> {
        self.get_slice(1).map(|s| s[0])
    }

    /// Get the next two bytes of data as a u16, using big-endian conversion
    /// Advances the position by 2
    /// Can fail with [MqttReaderError::InsufficientData]
    fn get_u16(&mut self) -> Result<u16> {
        let slice = self.get_slice(core::mem::size_of::<u16>())?;
        Ok(u16::from_be_bytes(slice.try_into().unwrap()))
    }

    /// Get the next four bytes of data as a u32, using big-endian conversion
    /// Advances the position by 4
    /// Can fail with [MqttReaderError::InsufficientData]
    fn get_u32(&mut self) -> Result<u32> {
        let slice = self.get_slice(core::mem::size_of::<u32>())?;
        Ok(u32::from_be_bytes(slice.try_into().unwrap()))
    }

    /// Get the next 1-4 bytes of data as a u32, using "variable byte integer"
    /// conversion as specified by MQTT v5
    /// Advances the position by 1 to 4 bytes
    /// Can fail with [MqttReaderError::InsufficientData] or [MqttReaderError::InvalidVariableByteIntegerEncoding]
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
                return Err(MqttReaderError::InvalidVariableByteIntegerEncoding);
            }
        }

        Ok(value)
    }

    /// Get the next bytes of data as a str, using the next two bytes as
    /// a string length, then the following bytes as utf8 string data,
    /// as specified by MQTT v5
    /// Advances the position by 2 or more bytes on success.
    /// If the decoded string contains any null characters, will fail with
    /// [MqttReaderError::NullCharacterInString].
    /// If the buffer data contains invalid utf8 data, will fail with
    /// [MqttReaderError::InvalidUtf8].
    /// Can fail with [MqttReaderError::InsufficientData],
    fn get_str(&mut self) -> Result<&'a str> {
        let len = self.get_u16()?;
        let slice = self.get_slice(len as usize)?;
        let s = core::str::from_utf8(slice)?;
        if s.contains("\0") {
            Err(MqttReaderError::NullCharacterInString)
        } else {
            Ok(s)
        }
    }

    /// Get the next bytes of data as a [StringPair], by attempting to read
    /// a name as a string, then a value as a string, as specified by MQTT v5
    /// Advances the position by 4 or more bytes on success.
    /// If either decoded string contains any null characters, will fail with
    /// [MqttReaderError::NullCharacterInString].
    /// If the buffer data contains invalid utf8 data for either strnig, will fail with
    /// [MqttReaderError::InvalidUtf8].
    /// Can fail with [MqttReaderError::InsufficientData],
    fn get_string_pair(&mut self) -> Result<StringPair<'a>> {
        let name = self.get_str()?;
        let value = self.get_str()?;
        Ok(StringPair::new(name, value))
    }

    /// Get the next bytes of data as a delimited binary data item,
    /// using the next two bytes as a data length, then the following
    /// bytes as the data itself, as specified by MQTT v5.
    /// Advances the position by 2 or more bytes on success.
    /// Can fail with [MqttReaderError::InsufficientData],
    fn get_binary_data(&mut self) -> Result<&'a [u8]> {
        let len = self.get_u16()?;
        self.get_slice(len as usize)
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
    fn get_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.position + len;
        if end > self.buf.len() {
            Err(MqttReaderError::InsufficientData)
        } else {
            let slice = &self.buf[self.position..end];
            self.position = end;
            Ok(slice)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(r.get_slice(1), Err(MqttReaderError::InsufficientData));
        assert_eq!(r.get_slice(2), Err(MqttReaderError::InsufficientData));

        // Can still get an empty slice
        let slice_empty = r.get_slice(0)?;
        assert_eq!(slice_empty, &[]);

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
        assert_eq!(r.get_slice(1), Err(MqttReaderError::InsufficientData));

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
        assert_eq!(r.get_u16(), Err(MqttReaderError::InsufficientData));

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
        assert_eq!(r.get_u32(), Err(MqttReaderError::InsufficientData));

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
            Err(MqttReaderError::InvalidVariableByteIntegerEncoding)
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
        let test_cases: &[(&[u8], MqttReaderError)] = &[
            // Character data MUST NOT include encodings of code points between U+D800 and U+DFFF
            // U+D800
            (
                &[0x00, 0x03, 0xED, 0xA0, 0x80],
                MqttReaderError::InvalidUtf8,
            ),
            // U+D900
            (
                &[0x00, 0x03, 0xED, 0xA4, 0x80],
                MqttReaderError::InvalidUtf8,
            ),
            // U+DFFF
            (
                &[0x00, 0x03, 0xED, 0xBF, 0xBF],
                MqttReaderError::InvalidUtf8,
            ),
            // A UTF-8 Encoded String MUST NOT include an encoding of the null character U+0000.
            (&[0x00, 0x01, 0x00], MqttReaderError::NullCharacterInString),
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

        assert_eq!(r.get_str(), Err(MqttReaderError::InsufficientData));

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

        assert_eq!(r.get_binary_data(), Err(MqttReaderError::InsufficientData));

        Ok(())
    }

    // TODO: Test for string pair
}
