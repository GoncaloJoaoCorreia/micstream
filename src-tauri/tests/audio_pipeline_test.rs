use micstream_lib::audio::{
    calculate_rms, check_host_virtual_driver_status, list_input_devices, list_output_devices,
    AdaptiveJitterBuffer, DynamicDriftResampler, DEFAULT_TARGET_WATERMARK_SAMPLES,
};
use micstream_lib::codec::{
    decode_pcm_i16_le_to_f32, encode_f32_to_pcm_i16_le, OpusAudioDecoder, OpusAudioEncoder,
    OPUS_FRAME_SIZE_SAMPLES, OPUS_SAMPLE_RATE,
};
use micstream_lib::protocol::{PacketHeader, PayloadType, HEADER_SIZE, MAGIC};

#[test]
fn test_device_enumeration_sanity() {
    let inputs = list_input_devices().expect("Failed to query input devices");
    let outputs = list_output_devices().expect("Failed to query output devices");

    println!(
        "Detected {} inputs, {} outputs",
        inputs.len(),
        outputs.len()
    );

    let status = check_host_virtual_driver_status();
    assert!(!status.driver_name.is_empty());
}

#[test]
fn test_audio_pipeline_opus_end_to_end() {
    // 1. Generate 5ms of 440 Hz audio at 48kHz (240 samples)
    let mut original_samples = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    for (i, s) in original_samples.iter_mut().enumerate() {
        let t = i as f32 / OPUS_SAMPLE_RATE as f32;
        *s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7;
    }

    let rms_input = calculate_rms(&original_samples);
    assert!(rms_input > 0.1);

    // 2. Encode with Opus LowDelay (5ms CELT)
    let mut encoder = OpusAudioEncoder::new().expect("Encoder init failed");
    let mut encoded_packet = vec![0u8; 512];
    let written_bytes = encoder
        .encode_float(&original_samples, &mut encoded_packet)
        .expect("Opus encoding failed");
    assert!(written_bytes > 0);

    // 3. Wrap in 16-byte protocol packet
    let header = PacketHeader::new(PayloadType::Opus, 1, 1000000, written_bytes as u16);
    let mut full_packet = vec![0u8; HEADER_SIZE + written_bytes];
    header.encode(&mut full_packet[..HEADER_SIZE]).unwrap();
    full_packet[HEADER_SIZE..].copy_from_slice(&encoded_packet[..written_bytes]);

    // 4. Decode protocol header
    let decoded_header = PacketHeader::decode(&full_packet[..HEADER_SIZE]).unwrap();
    assert_eq!(decoded_header.magic, MAGIC);
    assert_eq!(decoded_header.payload_type, PayloadType::Opus);
    assert_eq!(decoded_header.payload_length as usize, written_bytes);

    // 5. Decode with Opus LowDelay
    let mut decoder = OpusAudioDecoder::new().expect("Decoder init failed");
    let mut decoded_samples = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    let decoded_count = decoder
        .decode_float(
            Some(&full_packet[HEADER_SIZE..HEADER_SIZE + written_bytes]),
            &mut decoded_samples,
        )
        .expect("Opus decoding failed");
    assert_eq!(decoded_count, OPUS_FRAME_SIZE_SAMPLES);

    // 6. Push into Jitter Buffer
    let mut jitter_buffer = AdaptiveJitterBuffer::new(DEFAULT_TARGET_WATERMARK_SAMPLES);
    jitter_buffer.push_samples(&decoded_samples);
    assert_eq!(jitter_buffer.current_occupancy(), OPUS_FRAME_SIZE_SAMPLES);

    // 7. Pop from Jitter Buffer and Resample with Rubato
    let mut popped_samples = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    let popped_count = jitter_buffer.pop_samples(&mut popped_samples);
    assert_eq!(popped_count, OPUS_FRAME_SIZE_SAMPLES);

    let mut resampler =
        DynamicDriftResampler::new(OPUS_FRAME_SIZE_SAMPLES, 1.0001).expect("Resampler init failed");
    let resampled = resampler
        .process(&popped_samples)
        .expect("Resampling failed");
    assert!(!resampled.is_empty());

    let rms_output = calculate_rms(&resampled);
    assert!(rms_output > 0.1);
}

#[test]
fn test_audio_pipeline_raw_pcm_end_to_end() {
    let original_samples: Vec<f32> = (0..OPUS_FRAME_SIZE_SAMPLES)
        .map(|i| ((i as f32) / 240.0) * 0.5)
        .collect();

    // 1. Encode PCM f32 -> i16 LE bytes
    let mut pcm_bytes = vec![0u8; OPUS_FRAME_SIZE_SAMPLES * 2];
    let byte_len = encode_f32_to_pcm_i16_le(&original_samples, &mut pcm_bytes);
    assert_eq!(byte_len, OPUS_FRAME_SIZE_SAMPLES * 2);

    // 2. Wrap in protocol packet
    let header = PacketHeader::new(PayloadType::RawPcm, 2, 2000000, byte_len as u16);
    let mut packet = vec![0u8; HEADER_SIZE + byte_len];
    header.encode(&mut packet[..HEADER_SIZE]).unwrap();
    packet[HEADER_SIZE..].copy_from_slice(&pcm_bytes);

    // 3. Decode packet
    let decoded_header = PacketHeader::decode(&packet[..HEADER_SIZE]).unwrap();
    assert_eq!(decoded_header.payload_type, PayloadType::RawPcm);

    // 4. Decode PCM i16 LE bytes -> f32
    let mut reconstructed = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    let sample_count = decode_pcm_i16_le_to_f32(&packet[HEADER_SIZE..], &mut reconstructed);
    assert_eq!(sample_count, OPUS_FRAME_SIZE_SAMPLES);

    for (a, b) in original_samples.iter().zip(reconstructed.iter()) {
        assert!((a - b).abs() < 0.001);
    }
}
