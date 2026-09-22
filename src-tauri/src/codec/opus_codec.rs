use opus::{Application, Channels, Decoder, Encoder};
use thiserror::Error;

pub const OPUS_SAMPLE_RATE: u32 = 48000;
pub const OPUS_FRAME_SIZE_SAMPLES: usize = 240; // 5ms @ 48kHz

#[derive(Error, Debug)]
pub enum OpusCodecError {
    #[error("Opus initialization error: {0}")]
    InitError(#[from] opus::Error),
    #[error("Input buffer size mismatch: expected {expected} samples, got {actual}")]
    InvalidInputSize { expected: usize, actual: usize },
    #[error("Output buffer too small: capacity {capacity}, needed {needed}")]
    OutputTooSmall { capacity: usize, needed: usize },
}

pub struct OpusAudioEncoder {
    encoder: Encoder,
}

impl OpusAudioEncoder {
    pub fn new() -> Result<Self, OpusCodecError> {
        let mut encoder = Encoder::new(OPUS_SAMPLE_RATE, Channels::Mono, Application::LowDelay)?;
        // Complexity 5: optimized for real-time low CPU usage with pristine speech clarity
        encoder.set_complexity(5)?;
        encoder.set_bitrate(opus::Bitrate::Bits(96_000))?;
        Ok(Self { encoder })
    }

    /// Encodes 240 mono f32 samples into the provided byte buffer. Returns written byte length.
    pub fn encode_float(
        &mut self,
        input_samples: &[f32],
        out_bytes: &mut [u8],
    ) -> Result<usize, OpusCodecError> {
        if input_samples.len() != OPUS_FRAME_SIZE_SAMPLES {
            return Err(OpusCodecError::InvalidInputSize {
                expected: OPUS_FRAME_SIZE_SAMPLES,
                actual: input_samples.len(),
            });
        }

        let written = self.encoder.encode_float(input_samples, out_bytes)?;
        Ok(written)
    }

    /// Encodes 240 mono i16 samples into the provided byte buffer. Returns written byte length.
    pub fn encode_i16(
        &mut self,
        input_samples: &[i16],
        out_bytes: &mut [u8],
    ) -> Result<usize, OpusCodecError> {
        if input_samples.len() != OPUS_FRAME_SIZE_SAMPLES {
            return Err(OpusCodecError::InvalidInputSize {
                expected: OPUS_FRAME_SIZE_SAMPLES,
                actual: input_samples.len(),
            });
        }

        let written = self.encoder.encode(input_samples, out_bytes)?;
        Ok(written)
    }
}

pub struct OpusAudioDecoder {
    decoder: Decoder,
}

impl OpusAudioDecoder {
    pub fn new() -> Result<Self, OpusCodecError> {
        let decoder = Decoder::new(OPUS_SAMPLE_RATE, Channels::Mono)?;
        Ok(Self { decoder })
    }

    /// Decodes an Opus packet into mono f32 samples.
    /// If packet is None, triggers Packet Loss Concealment (PLC).
    pub fn decode_float(
        &mut self,
        packet: Option<&[u8]>,
        out_samples: &mut [f32],
    ) -> Result<usize, OpusCodecError> {
        let input = packet.unwrap_or(&[]);
        let decoded = self.decoder.decode_float(input, out_samples, false)?;
        Ok(decoded)
    }

    /// Decodes an Opus packet into mono i16 samples.
    /// If packet is None, triggers Packet Loss Concealment (PLC).
    pub fn decode_i16(
        &mut self,
        packet: Option<&[u8]>,
        out_samples: &mut [i16],
    ) -> Result<usize, OpusCodecError> {
        let input = packet.unwrap_or(&[]);
        let decoded = self.decoder.decode(input, out_samples, false)?;
        Ok(decoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opus_encode_decode_roundtrip() {
        let mut enc = OpusAudioEncoder::new().expect("Failed to create encoder");
        let mut dec = OpusAudioDecoder::new().expect("Failed to create decoder");

        // Generate 440 Hz test sine wave at 48 kHz (240 samples = 5ms)
        let mut input = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
        for (i, sample) in input.iter_mut().enumerate() {
            let t = i as f32 / OPUS_SAMPLE_RATE as f32;
            *sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.8;
        }

        let mut packet = [0u8; 512];
        let bytes_written = enc
            .encode_float(&input, &mut packet)
            .expect("Encoding failed");
        assert!(bytes_written > 0);

        let mut decoded = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
        let samples_decoded = dec
            .decode_float(Some(&packet[..bytes_written]), &mut decoded)
            .expect("Decoding failed");
        assert_eq!(samples_decoded, OPUS_FRAME_SIZE_SAMPLES);

        // Test PLC (missing packet)
        let mut plc_decoded = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
        let plc_samples = dec
            .decode_float(None, &mut plc_decoded)
            .expect("PLC decoding failed");
        assert_eq!(plc_samples, OPUS_FRAME_SIZE_SAMPLES);
    }
}
