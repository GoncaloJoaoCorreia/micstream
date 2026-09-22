use byteorder::{BigEndian, ByteOrder};
use thiserror::Error;

pub const MAGIC: u16 = 0x4D53; // 'M', 'S'
pub const CURRENT_VERSION: u8 = 0x01;
pub const HEADER_SIZE: usize = 20;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PacketError {
    #[error("Buffer too small: expected at least {expected} bytes, got {actual}")]
    BufferTooSmall { expected: usize, actual: usize },
    #[error("Invalid magic bytes: 0x{0:04X}, expected 0x4D53")]
    InvalidMagic(u16),
    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(u8),
    #[error("Unknown payload type: 0x{0:02X}")]
    UnknownPayloadType(u8),
    #[error("Payload length mismatch: header specifies {specified} bytes, got {available}")]
    PayloadLengthMismatch { specified: usize, available: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PayloadType {
    Opus = 0x00,
    RawPcm = 0x01,
    Ping = 0x10,
    Pong = 0x11,
}

impl TryFrom<u8> for PayloadType {
    type Error = PacketError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(PayloadType::Opus),
            0x01 => Ok(PayloadType::RawPcm),
            0x10 => Ok(PayloadType::Ping),
            0x11 => Ok(PayloadType::Pong),
            other => Err(PacketError::UnknownPayloadType(other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    pub magic: u16,
    pub version: u8,
    pub payload_type: PayloadType,
    pub sequence_number: u32,
    pub timestamp_us: u64,
    pub payload_length: u16,
    pub reserved: u16,
}

impl PacketHeader {
    pub fn new(
        payload_type: PayloadType,
        sequence_number: u32,
        timestamp_us: u64,
        payload_length: u16,
    ) -> Self {
        Self {
            magic: MAGIC,
            version: CURRENT_VERSION,
            payload_type,
            sequence_number,
            timestamp_us,
            payload_length,
            reserved: 0,
        }
    }

    pub fn encode(&self, out: &mut [u8]) -> Result<(), PacketError> {
        if out.len() < HEADER_SIZE {
            return Err(PacketError::BufferTooSmall {
                expected: HEADER_SIZE,
                actual: out.len(),
            });
        }

        BigEndian::write_u16(&mut out[0..2], self.magic);
        out[2] = self.version;
        out[3] = self.payload_type as u8;
        BigEndian::write_u32(&mut out[4..8], self.sequence_number);
        BigEndian::write_u64(&mut out[8..16], self.timestamp_us);
        BigEndian::write_u16(&mut out[16..18], self.payload_length);
        BigEndian::write_u16(&mut out[18..20], self.reserved);

        Ok(())
    }

    pub fn decode(src: &[u8]) -> Result<Self, PacketError> {
        if src.len() < HEADER_SIZE {
            return Err(PacketError::BufferTooSmall {
                expected: HEADER_SIZE,
                actual: src.len(),
            });
        }

        let magic = BigEndian::read_u16(&src[0..2]);
        if magic != MAGIC {
            return Err(PacketError::InvalidMagic(magic));
        }

        let version = src[2];
        if version != CURRENT_VERSION {
            return Err(PacketError::UnsupportedVersion(version));
        }

        let payload_type = PayloadType::try_from(src[3])?;
        let sequence_number = BigEndian::read_u32(&src[4..8]);
        let timestamp_us = BigEndian::read_u64(&src[8..16]);
        let payload_length = BigEndian::read_u16(&src[16..18]);
        let reserved = BigEndian::read_u16(&src[18..20]);

        Ok(Self {
            magic,
            version,
            payload_type,
            sequence_number,
            timestamp_us,
            payload_length,
            reserved,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_header_roundtrip() {
        let header = PacketHeader::new(PayloadType::Opus, 42, 1234567890123, 240);
        let mut buf = [0u8; 20];
        header.encode(&mut buf).unwrap();

        let decoded = PacketHeader::decode(&buf).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    fn test_invalid_magic() {
        let mut buf = [0u8; 20];
        BigEndian::write_u16(&mut buf[0..2], 0x1234);
        buf[2] = CURRENT_VERSION;
        buf[3] = 0x00;

        assert_eq!(
            PacketHeader::decode(&buf),
            Err(PacketError::InvalidMagic(0x1234))
        );
    }

    #[test]
    fn test_buffer_too_small() {
        let buf = [0u8; 10];
        assert_eq!(
            PacketHeader::decode(&buf),
            Err(PacketError::BufferTooSmall {
                expected: HEADER_SIZE,
                actual: 10
            })
        );
    }
}
