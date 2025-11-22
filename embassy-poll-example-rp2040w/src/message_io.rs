use embedded_io_async::Read;
use mountain_mqtt::{
    codec::mqtt_reader::{MqttBufReader, MqttReader},
    data::packet_type::PacketType,
    error::PacketReadError,
};
#[derive(Clone, Copy)]
pub struct PacketBin<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> PacketBin<N> {
    pub fn new(msg_data: &[u8]) -> Result<Self, Error> {
        let len = msg_data.len();
        if len > N {
            Err(Error::PacketBinTooLarge)
        } else {
            // TODO: Is it worth doing this without initialising the array where we are just going
            // to overwrite it?
            let mut buf = [0; N];
            buf[0..len].clone_from_slice(msg_data);
            Ok(Self { buf, len })
        }
    }

    pub fn msg_data(&self) -> &[u8] {
        &self.buf[0..self.len]
    }
}

pub enum Error {
    ReadExact,
    PacketBinTooLarge,
    PacketRead(PacketReadError),
}

pub async fn receive_packet_bin<R, const N: usize>(read: &mut R) -> Result<PacketBin<N>, Error>
where
    R: Read,
{
    let mut buf = [0; N];
    let n = receive_packet_buf(read, &mut buf).await?;
    PacketBin::new(&buf[0..n])
}

pub async fn receive_packet_buf<R, const N: usize>(
    read: &mut R,
    buf: &mut [u8; N],
) -> Result<usize, Error>
where
    R: Read,
{
    let mut position: usize = 0;

    read.read_exact(&mut buf[0..1])
        .await
        .map_err(|_| Error::ReadExact)?;
    position += 1;

    // Check first header byte is valid, if not we can error early without
    // trying to read the rest of a packet
    if !PacketType::is_valid_first_header_byte(buf[0]) {
        return Err(Error::PacketRead(PacketReadError::InvalidPacketType));
    }

    // We will read up to 4 bytes into buffer as variable u32

    // First byte always exists
    read.read_exact(&mut buf[position..position + 1])
        .await
        .map_err(|_| Error::ReadExact)?;
    position += 1;

    // Read up to 3 more bytes looking for the end of the encoded length
    for _extra in 0..3 {
        if buf[position - 1] & 128 == 0 {
            break;
        } else {
            read.read_exact(&mut buf[position..position + 1])
                .await
                .map_err(|_| Error::ReadExact)?;
            position += 1;
        }
    }

    // Error if we didn't see the end of the length
    if buf[position - 1] & 128 != 0 {
        return Err(Error::PacketRead(
            PacketReadError::InvalidVariableByteIntegerEncoding,
        ));
    }

    // We have a valid length, decode it
    let remaining_length = {
        let mut r = MqttBufReader::new(&buf[1..position]);
        r.get_variable_u32().map_err(Error::PacketRead)?
    } as usize;

    // If packet will not fit in buffer, error
    if position + remaining_length > buf.len() {
        return Err(Error::PacketRead(PacketReadError::PacketTooLargeForBuffer));
    }

    // Read the rest of the packet
    read.read_exact(&mut buf[position..position + remaining_length])
        .await
        .map_err(|_| Error::ReadExact)?;
    position += remaining_length;

    Ok(position)
}

// pub fn receive_message<R, const P: usize, const W: usize, const S: usize>(
//     read: &mut R,
// ) -> Result<PacketGeneric<'_, P, W, S>, ReadExactError<R::Error>>
// where
//     R: Read,
// {
// }
