#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Error {
    MalformedPacket,
    ConnectionReceive,
    ConnectionSend,
    ConnectionReceiveInvalidData,
}
