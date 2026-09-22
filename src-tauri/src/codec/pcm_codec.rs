use byteorder::{ByteOrder, LittleEndian};

pub const PCM_SAMPLE_RATE: u32 = 48000;
pub const BYTES_PER_SAMPLE: usize = 2; // 16-bit signed integer

/// Converts normalized f32 audio samples (-1.0 to 1.0) to 16-bit signed little-endian PCM bytes.
pub fn encode_f32_to_pcm_i16_le(samples: &[f32], out_bytes: &mut [u8]) -> usize {
    let count = samples.len().min(out_bytes.len() / BYTES_PER_SAMPLE);
    for i in 0..count {
        let clamped = samples[i].clamp(-1.0, 1.0);
        let sample_i16 = if clamped >= 0.0 {
            (clamped * i16::MAX as f32) as i16
        } else {
            (clamped * -(i16::MIN as f32)) as i16
        };
        LittleEndian::write_i16(&mut out_bytes[i * 2..(i + 1) * 2], sample_i16);
    }
    count * BYTES_PER_SAMPLE
}

/// Decodes 16-bit signed little-endian PCM bytes into normalized f32 audio samples (-1.0 to 1.0).
pub fn decode_pcm_i16_le_to_f32(bytes: &[u8], out_samples: &mut [f32]) -> usize {
    let sample_count = (bytes.len() / BYTES_PER_SAMPLE).min(out_samples.len());
    for i in 0..sample_count {
        let sample_i16 = LittleEndian::read_i16(&bytes[i * 2..(i + 1) * 2]);
        out_samples[i] = if sample_i16 >= 0 {
            sample_i16 as f32 / i16::MAX as f32
        } else {
            sample_i16 as f32 / -(i16::MIN as f32)
        };
    }
    sample_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcm_conversion_roundtrip() {
        let original = vec![-1.0f32, -0.5, 0.0, 0.5, 1.0];
        let mut bytes = vec![0u8; original.len() * BYTES_PER_SAMPLE];
        let written = encode_f32_to_pcm_i16_le(&original, &mut bytes);
        assert_eq!(written, 10);

        let mut reconstructed = vec![0.0f32; original.len()];
        let decoded_count = decode_pcm_i16_le_to_f32(&bytes, &mut reconstructed);
        assert_eq!(decoded_count, 5);

        for (orig, recon) in original.iter().zip(reconstructed.iter()) {
            assert!((orig - recon).abs() < 0.001);
        }
    }
}
