use std::{
    pin::Pin,
    task::{Context, Poll},
};

use crate::util::Result;

use bytes::Buf;
use futures::{Sink, Stream};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use tokio_util::codec::{Decoder, Encoder, Framed};
use crate::util::AdbServerError;

const MAX: usize = 8 * 1024 * 1024;
const ADB_SERVER_PROTOCOL_HEADER_LENGTH: usize = 24;

/// ADB protocol command.
#[derive(Clone, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum Command {
    /// Connect Command
    A_CNXN = 0x4e584e43,
    /// Auth Command
    A_AUTH = 0x48545541,
    /// Open Command
    A_OPEN = 0x4e45504f,
    /// Okay Command
    A_OKAY = 0x59414b4f,
    /// Close Command
    A_CLSE = 0x45534c43,
    /// Write Command
    A_WRTE = 0x45545257,

}

/// ADB protocol header.
pub struct AdbHeader {
    /// Command identifier.
    pub command: Command,
    /// First argument.
    pub arg0: u32,
    /// Second argument.
    pub arg1: u32,
    /// Length of the data payload.
    pub data_length: u32,
    /// Checksum of the data payload.
    pub data_checksum: u32,
    /// Magic value.
    pub magic: u32,
}

impl std::fmt::Debug for AdbHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AdbHeader:\n{}",
            pretty_hex::pretty_hex(&self.to_bytes())
        )
    }
}

impl AdbHeader {
    /// Convert the header to a byte array.
    pub fn to_bytes(&self) -> [u8; 24] {
        let mut arr = [0u8; 24];
        let cmd: u32 = self.command.clone().into();
        arr[0..4].copy_from_slice(&cmd.to_le_bytes());
        arr[4..8].copy_from_slice(&self.arg0.to_le_bytes());
        arr[8..12].copy_from_slice(&self.arg1.to_le_bytes());
        arr[12..16].copy_from_slice(&self.data_length.to_le_bytes());
        arr[16..20].copy_from_slice(&self.data_checksum.to_le_bytes());
        arr[20..24].copy_from_slice(&self.magic.to_le_bytes());
        arr
    }

    /// Create a header from a byte array.
    pub fn from_bytes(data: &[u8]) -> Result<AdbHeader> {
        let command = u32::from_le_bytes(data[0..4].try_into()?).try_into().map_err(|e| {
            AdbServerError::Error(format!("Invalid command: {}", e))
        })?;

        let arg0 = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let arg1 = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let data_length = u32::from_le_bytes(data[12..16].try_into().unwrap());
        let data_checksum = u32::from_le_bytes(data[16..20].try_into().unwrap());
        let magic = u32::from_le_bytes(data[20..24].try_into().unwrap());

        Ok(AdbHeader {
            command,
            arg0,
            arg1,
            data_length,
            data_checksum,
            magic,
        })
    }
}

/// ADB packet.
pub struct AdbPacket {
    /// ADB header.
    pub header: AdbHeader,
    /// ADB data.
    pub data: Vec<u8>,
}

impl std::fmt::Debug for AdbPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}\nAdb Payload:\n{}",
            self.header,
            pretty_hex::pretty_hex(&self.data)
        )
    }
}

/// ADB server codec
#[derive(Debug)]
pub struct AdbServerCodec {}

impl AdbServerCodec {
    /// Create a new AdbServerCodec instance
    pub fn new() -> AdbServerCodec {
        AdbServerCodec {}
    }
}

impl Decoder for AdbServerCodec {
    type Item = AdbPacket;
    type Error = AdbServerError;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>> {
        if src.len() < ADB_SERVER_PROTOCOL_HEADER_LENGTH {
            return Ok(None);
        }

        let header = AdbHeader::from_bytes(&src[0..ADB_SERVER_PROTOCOL_HEADER_LENGTH])?;
        let length = ADB_SERVER_PROTOCOL_HEADER_LENGTH + header.data_length as usize;

        // Check that the length is not too large to avoid a denial of
        // service attack where the server runs out of memory.
        if length > MAX {
            return Err(AdbServerError::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", length),
            )));
        }

        if src.len() < length {
            // The full string has not yet arrived.
            //
            // We reserve more space in the buffer. This is not strictly
            // necessary, but is a good idea performance-wise.
            src.reserve(length - src.len());

            // We inform the Framed that we need more bytes to form the next
            // frame.
            return Ok(None);
        }

        let data = src[ADB_SERVER_PROTOCOL_HEADER_LENGTH..length].to_vec();
        src.advance(length);

        Ok(Some(AdbPacket { header, data }))
    }
}

impl Encoder<AdbPacket> for AdbServerCodec {
    type Error = AdbServerError;

    fn encode(&mut self, packet: AdbPacket, dst: &mut bytes::BytesMut) -> Result<()> {
        // Don't send a string if it is longer than the other end will
        // accept.
        let length = ADB_SERVER_PROTOCOL_HEADER_LENGTH + (packet.header.data_length as usize);
        if length > MAX {
            return Err(AdbServerError::IOError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", length),
            )));
        }

        // Reserve space in the buffer.
        dst.reserve(length - dst.len());

        dst.extend_from_slice(&packet.header.to_bytes());
        dst.extend_from_slice(&packet.data);

        Ok(())
    }
}

/// Adb Server Protocol Connection
#[derive(Debug)]
pub struct AdbServerProtocolConnection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    framed: Framed<S, AdbServerCodec>,
}

impl<S> AdbServerProtocolConnection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    /// Create a new AdbServerProtocolConnection instance
    pub fn new(stream: S) -> AdbServerProtocolConnection<S>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let framed = Framed::new(stream, AdbServerCodec::new());

        return AdbServerProtocolConnection { framed };
    }
}

impl<S> Stream for AdbServerProtocolConnection<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    type Item = Result<AdbPacket>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.framed).poll_next(cx)
    }
}

impl<S> Sink<AdbPacket> for AdbServerProtocolConnection<S>
where
S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    type Error = AdbServerError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<core::result::Result<(), Self::Error>> {
        Pin::new(&mut self.framed).poll_ready(cx)
    }

    fn start_send(mut self: Pin<&mut Self>, item: AdbPacket) -> core::result::Result<(), Self::Error> {
        Pin::new(&mut self.framed).start_send(item)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<core::result::Result<(), Self::Error>> {
        Pin::new(&mut self.framed).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<core::result::Result<(), Self::Error>> {
        Pin::new(&mut self.framed).poll_close(cx)
    }
}