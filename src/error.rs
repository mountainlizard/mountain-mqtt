#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Error {
    /// Data contained an invalid fixed header first byte value that could not be parsed
    InvalidFixedHeaderFirstByte,
}
