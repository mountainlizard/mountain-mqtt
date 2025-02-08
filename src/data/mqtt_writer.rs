use super::{DATA_MAX_LEN, VARIABLE_BYTE_INTEGER_MAX_VALUE};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MqttWriterError {
    /// On attempt to put data that will not fit in buffer
    Overflow,
    /// On attempt to put a string containing a null character (which is not valid in an MQTT message)
    NullCharacterInString,
    /// On attempt to put a u32 value as a variable byte integer, where the value is too large to encode
    VariableByteIntegerTooLarge,
    /// On attempt to put a string or binary data where the encoded form is too many bytes to encode
    DataTooLarge,
}

pub type Result<A> = core::result::Result<A, MqttWriterError>;

/// A writer putting data to an underlying buffer in MQTT v5 encodings.
/// Each `put` method puts encoded data to the buffer, advancing its position.
/// If a method fails (returns an [Err]), then partial data may have been
/// written before the error was detected. Since an error always indicates
/// invalid data, there's no reason to allow recovering, and the writer/data
/// should not be used further.
pub trait MqttWriter<'a> {
    /// Put the whole of a slice as raw data
    /// This generally should not be used directly - it is
    /// used by the other put methods, which handle checking
    /// and encoding different input data types as raw slices.
    /// Advances the position by length of slice
    /// Can fail with [MqttWriterError::Overflow]
    fn put_slice(&mut self, slice: &[u8]) -> Result<()>;

    /// Put the next byte of data as a u8
    /// Advances the position by 1
    /// Can fail with [MqttWriterError::Overflow]
    fn put_u8(&mut self, n: u8) -> Result<()> {
        self.put_slice(&[n])
    }

    /// Put the next two bytes of data as a u16, using big-endian conversion
    /// Advances the position by 2
    /// Can fail with [MqttWriterError::Overflow]
    fn put_u16(&mut self, n: u16) -> Result<()> {
        let data = n.to_be_bytes();
        self.put_slice(&data)
    }

    /// Put the next four bytes of data as a u32, using big-endian conversion
    /// Advances the position by 4
    /// Can fail with [MqttWriterError::Overflow]
    fn put_u32(&mut self, n: u32) -> Result<()> {
        let data = n.to_be_bytes();
        self.put_slice(&data)
    }

    /// Put the next 1-4 bytes of data as a u32, using "variable byte integer"
    /// conversion as specified by MQTT v5
    /// Advances the position by 1 to 4 bytes depending on the
    /// size of the value
    /// Fails with [MqttWriterError::VariableByteIntegerTooLarge] if value is
    /// larger than [VARIABLE_BYTE_INTEGER_MAX_VALUE]
    /// Can fail with [MqttWriterError::Overflow]
    /// In case of a failure, the position will be advanced by however many bytes
    /// were written in an attempt to encode.
    /// Returns the number of bytes written
    fn put_variable_u32(&mut self, mut n: u32) -> Result<usize> {
        if n > VARIABLE_BYTE_INTEGER_MAX_VALUE {
            Err(MqttWriterError::VariableByteIntegerTooLarge)
        } else {
            let mut len = 0;
            loop {
                let mut encoded_byte = (n % 128) as u8;
                n /= 128;
                // if there are more data to encode, set the top bit of this byte
                if n > 0 {
                    encoded_byte |= 128
                }
                self.put_u8(encoded_byte)?;
                len += 1;
                if n == 0 {
                    break;
                }
            }
            Ok(len)
        }
    }

    /// Put the next bytes of data as a str, using the next two bytes as
    /// a string length, then the following bytes as utf8 string data,
    /// as specified by MQTT v5
    /// To comply with MQTT v5, the string will be checked for null characters,
    /// and if any are found this will fail with [MqttWriterError::NullCharacterInString]. If the
    /// string is longer than [DATA_MAX_LEN] this will fail with [MqttWriterError::DataTooLarge]
    /// Advances the position by 2 or more bytes on success.
    /// Can fail with [MqttWriterError::Overflow]
    /// In case of a failure, the position will be advanced by however many bytes
    /// were written in an attempt to encode.
    fn put_str(&mut self, s: &str) -> Result<usize> {
        let len = s.len();
        if len > DATA_MAX_LEN {
            Err(MqttWriterError::DataTooLarge)
        } else if s.contains("\0") {
            Err(MqttWriterError::NullCharacterInString)
        } else {
            self.put_u16(len as u16)?;
            self.put_slice(s.as_bytes())?;
            Ok(len + 2)
        }
    }

    /// Put the next bytes of data as a delimited binary data item,
    /// using the next two bytes as a data length, then the following
    /// bytes as the data itself, as specified by MQTT v5
    /// If the data is longer than [DATA_MAX_LEN] this will fail with [MqttWriterError::DataTooLarge]
    /// Advances the position by 2 or more bytes on success.
    /// Can fail with [MqttWriterError::Overflow]
    /// In case of a failure, the position will be advanced by however many bytes
    /// were written in an attempt to encode.
    fn put_binary_data(&mut self, data: &[u8]) -> Result<usize> {
        let len = data.len();
        if len > DATA_MAX_LEN {
            Err(MqttWriterError::DataTooLarge)
        } else {
            self.put_u16(len as u16)?;
            self.put_slice(data)?;
            Ok(len + 2)
        }
    }
}

pub struct MqttBufWriter<'a> {
    buf: &'a mut [u8],
    position: usize,
}

impl<'a> MqttBufWriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, position: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.position
    }

    pub fn position(&self) -> usize {
        self.position
    }
}

impl<'a> MqttWriter<'a> for MqttBufWriter<'a> {
    fn put_slice(&mut self, slice: &[u8]) -> Result<()> {
        let end = self.position + slice.len();
        if end > self.buf.len() {
            Err(MqttWriterError::Overflow)
        } else {
            self.buf[self.position..end].copy_from_slice(slice);
            self.position = end;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl MqttBufWriter<'_> {
        fn assert_contents(&self, expected: &[u8]) {
            assert_eq!(&self.buf[0..self.position], expected);
        }
    }

    #[test]
    fn mqtt_buf_writer_can_put_slices() -> Result<()> {
        let mut buf = [0u8; 4];
        let mut r = MqttBufWriter::new(&mut buf);

        assert_eq!(4, r.remaining());

        r.put_slice(&[1u8])?;
        assert_eq!(1, r.position());
        assert_eq!(3, r.remaining());
        r.assert_contents(&[1u8]);

        r.put_slice(&[2u8, 3])?;
        assert_eq!(3, r.position());
        assert_eq!(1, r.remaining());
        r.assert_contents(&[1u8, 2, 3]);

        r.put_slice(&[4u8])?;
        assert_eq!(4, r.position());
        assert_eq!(0, r.remaining());
        r.assert_contents(&[1u8, 2, 3, 4]);

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_errors_on_overflow() -> Result<()> {
        let mut buf = [0u8; 4];
        let mut r = MqttBufWriter::new(&mut buf);

        assert_eq!(4, r.remaining());

        r.put_slice(&[1u8])?;
        r.put_slice(&[2u8, 3])?;
        r.put_slice(&[4u8])?;

        assert_eq!(0, r.remaining());

        assert_eq!(r.put_slice(&[0u8]), Err(MqttWriterError::Overflow));
        assert_eq!(r.put_slice(&[0u8, 1]), Err(MqttWriterError::Overflow));

        // Can still put an empty slice
        assert_eq!(r.put_slice(&[]), Ok(()));

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_can_put_u8() -> Result<()> {
        let mut buf = [0u8; 7];
        let mut r = MqttBufWriter::new(&mut buf);

        let values = [0u8, 1, 2, 3, 4, 128, 255];

        let mut expected_position = 0;
        for value in values.iter() {
            r.put_u8(*value)?;
            expected_position += 1;
            r.assert_contents(&values[0..expected_position]);
            assert_eq!(r.position(), expected_position);
            assert_eq!(r.remaining(), 7 - expected_position);
        }

        // We've filled the buffer
        assert_eq!(0, r.remaining());

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_can_put_u16() -> Result<()> {
        let mut buf = [0u8; 10];
        let mut r = MqttBufWriter::new(&mut buf);

        let values = [0u16, 1, 255, 256, 65535];
        let expected: [u8; 10] = [0, 0, 0, 1, 0, 255, 1, 0, 255, 255];

        let mut expected_position = 0;
        for value in values.iter() {
            r.put_u16(*value)?;
            expected_position += 2;
            r.assert_contents(&expected[0..expected_position]);
            assert_eq!(r.position(), expected_position);
            assert_eq!(r.remaining(), 10 - expected_position);
        }

        // We've filled the buffer
        assert_eq!(0, r.remaining());

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_can_put_u32() -> Result<()> {
        let mut buf = [0u8; 36];
        let mut r = MqttBufWriter::new(&mut buf);

        let values: [u32; 9] = [0, 1, 255, 256, 65535, 65536, 16777215, 16777216, 4294967295];
        let expected: [u8; 36] = [
            0, 0, 0, 0, // 0
            0, 0, 0, 1, // 1
            0, 0, 0, 255, // 255
            0, 0, 1, 0, // 256
            0, 0, 255, 255, // 65535
            0, 1, 0, 0, // 65536
            0, 255, 255, 255, // 16777215
            1, 0, 0, 0, // 16777216
            255, 255, 255, 255, // 4294967295
        ];

        let mut expected_position = 0;
        for value in values.iter() {
            r.put_u32(*value)?;
            expected_position += 4;
            r.assert_contents(&expected[0..expected_position]);
            assert_eq!(r.position(), expected_position);
            assert_eq!(r.remaining(), 36 - expected_position);
        }

        // We've filled the buffer
        assert_eq!(0, r.remaining());

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_can_put_variable_u32_and_gives_correct_len() -> Result<()> {
        let mut buf = [0u8; 10];

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

        for (value, encoded) in test_cases.iter() {
            buf.fill(0);
            let mut r = MqttBufWriter::new(&mut buf[0..encoded.len()]);

            let len = r.put_variable_u32(*value)?;
            assert_eq!(len, encoded.len());
            assert_eq!(0, r.remaining());
            r.assert_contents(encoded);
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_errors_on_variable_u32_too_large() -> Result<()> {
        let mut buf = [0u8; 4];
        let mut r = MqttBufWriter::new(&mut buf);

        assert_eq!(r.put_variable_u32(VARIABLE_BYTE_INTEGER_MAX_VALUE), Ok(4));
        assert_eq!(
            r.put_variable_u32(VARIABLE_BYTE_INTEGER_MAX_VALUE + 1),
            Err(MqttWriterError::VariableByteIntegerTooLarge)
        );
        assert_eq!(
            r.put_variable_u32(VARIABLE_BYTE_INTEGER_MAX_VALUE + 2),
            Err(MqttWriterError::VariableByteIntegerTooLarge)
        );

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_can_put_str() -> Result<()> {
        let mut buf = [0u8; 10];

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

        for (s, encoded) in test_cases.iter() {
            buf.fill(0);
            let mut r = MqttBufWriter::new(&mut buf[0..encoded.len()]);

            let len = r.put_str(s)?;
            assert_eq!(len, encoded.len());
            assert_eq!(0, r.remaining());
            r.assert_contents(encoded);
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_reader_errors_on_invalid_strings() -> Result<()> {
        let mut buf = [0u8; 10];

        let test_cases: &[(&str, MqttWriterError)] = &[
            // A UTF-8 Encoded String MUST NOT include an encoding of the null character U+0000.
            ("\0", MqttWriterError::NullCharacterInString),
            ("A\0", MqttWriterError::NullCharacterInString),
            ("\0A", MqttWriterError::NullCharacterInString),
            ("A\0A", MqttWriterError::NullCharacterInString),
            ("\0\0", MqttWriterError::NullCharacterInString),
            // Note - no tests for badly-formed utf8, e.g. containing code points between
            // U+D800 and U+DFFF, since Rust strings are guaranteed to be valid. We only need
            // to test this on the reader side where we're accepting arbitrary binary data
        ];

        for (s, err) in test_cases.iter() {
            buf.fill(0);
            let mut r = MqttBufWriter::new(&mut buf);

            assert_eq!(r.put_str(s), Err(*err));
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_errors_on_overflow_for_string() -> Result<()> {
        // Write a string that just fits in 5 bytes (2 for u16 length, 3 for utf8 of "AAA")
        let mut buf = [0u8; 5];
        let mut r = MqttBufWriter::new(&mut buf);
        let len = r.put_str("AAA")?;
        assert_eq!(len, 5);

        // Now check this fails with one byte less buffer
        let mut buf = [0u8; 4];
        let mut r = MqttBufWriter::new(&mut buf);
        assert_eq!(r.put_str("AAA"), Err(MqttWriterError::Overflow));

        // Check another case
        let mut buf = [0u8; 6];
        let mut r = MqttBufWriter::new(&mut buf);
        assert_eq!(r.put_str("AAAAA"), Err(MqttWriterError::Overflow));

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_errors_if_encoded_string_too_large() -> Result<()> {
        // Succeed writing maximum encoded-length string
        // Note encoded length is utf8 data length plus 2
        let mut buf = [0u8; DATA_MAX_LEN + 2];
        // 0x41 is "A", we know this encodes from/to one byte
        let data = [0x41; DATA_MAX_LEN];
        let s = core::str::from_utf8(&data).unwrap();
        let mut r = MqttBufWriter::new(&mut buf);
        let len = r.put_str(s)?;
        assert_eq!(len, DATA_MAX_LEN + 2);

        // Fail writing one more than maximum data size
        let mut buf = [0u8; DATA_MAX_LEN + 1 + 2];
        let data = [0x41; DATA_MAX_LEN + 1];
        let s = core::str::from_utf8(&data).unwrap();
        let mut r = MqttBufWriter::new(&mut buf);
        assert_eq!(r.put_str(s), Err(MqttWriterError::DataTooLarge));

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_can_put_binary_data() -> Result<()> {
        let mut buf = [0u8; 10];

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

        for (data, encoded) in test_cases.iter() {
            // fill with non-zero for this one, since the first case actually encodes to all zeros
            buf.fill(0xFF);
            let mut r = MqttBufWriter::new(&mut buf[0..encoded.len()]);

            let len = r.put_binary_data(data)?;
            assert_eq!(len, encoded.len());
            assert_eq!(0, r.remaining());
            r.assert_contents(encoded);
        }

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_errors_on_overflow_for_binary_data() -> Result<()> {
        // Write data that just fits in 5 bytes (2 for u16 length, 3 data bytes)
        let mut buf = [0u8; 5];
        let mut r = MqttBufWriter::new(&mut buf);
        let len = r.put_binary_data(&[1, 2, 3])?;
        assert_eq!(len, 5);

        // Now check this fails with one byte less buffer
        let mut buf = [0u8; 4];
        let mut r = MqttBufWriter::new(&mut buf);
        assert_eq!(
            r.put_binary_data(&[1, 2, 3]),
            Err(MqttWriterError::Overflow)
        );

        // Check another case
        let mut buf = [0u8; 6];
        let mut r = MqttBufWriter::new(&mut buf);
        assert_eq!(
            r.put_binary_data(&[1, 2, 3, 4, 5]),
            Err(MqttWriterError::Overflow)
        );

        Ok(())
    }

    #[test]
    fn mqtt_buf_writer_errors_if_binary_data_too_large() -> Result<()> {
        // Succeed writing maximum data size
        // Note encoded length is data length plus 2
        let mut buf = [0u8; DATA_MAX_LEN + 2];
        let data = [0xFF; DATA_MAX_LEN];
        let mut r = MqttBufWriter::new(&mut buf);
        let len = r.put_binary_data(&data)?;
        assert_eq!(len, DATA_MAX_LEN + 2);

        // Fail writing one more than maximum data size
        let mut buf = [0u8; DATA_MAX_LEN + 1 + 2];
        let data = [0xFF; DATA_MAX_LEN + 1];
        let mut r = MqttBufWriter::new(&mut buf);
        assert_eq!(r.put_binary_data(&data), Err(MqttWriterError::DataTooLarge));

        Ok(())
    }
}
