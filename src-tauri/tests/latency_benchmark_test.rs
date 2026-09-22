use micstream_lib::audio::{
    AdaptiveJitterBuffer, DynamicDriftResampler, DEFAULT_TARGET_WATERMARK_SAMPLES,
};
use micstream_lib::codec::{
    decode_pcm_i16_le_to_f32, encode_f32_to_pcm_i16_le, OpusAudioDecoder, OpusAudioEncoder,
    OPUS_FRAME_SIZE_SAMPLES, OPUS_SAMPLE_RATE,
};
use micstream_lib::net::{UdpReceiver, UdpSender};
use micstream_lib::protocol::{PacketHeader, PayloadType, HEADER_SIZE};
use std::net::SocketAddr;
use std::time::Instant;

const FRAME_DURATION_MS: f32 = 5.0; // 240 samples @ 48kHz = 5.0ms
const TARGET_WATERMARK_MS: f32 = 5.0; // 5ms jitter buffer delay
const MAX_ALLOWED_OPUS_LATENCY_MS: f32 = 25.0; // SLA: sub-25ms
const MAX_ALLOWED_PCM_LATENCY_MS: f32 = 16.0; // SLA: sub-16ms

#[test]
fn test_subsystem_cpu_processing_benchmarks() {
    let iterations = 500;

    let test_samples: Vec<f32> = (0..OPUS_FRAME_SIZE_SAMPLES)
        .map(|i| {
            let t = i as f32 / OPUS_SAMPLE_RATE as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7
        })
        .collect();

    // 1. Opus Encoder & Decoder Benchmark
    let mut encoder = OpusAudioEncoder::new().expect("Opus encoder init failed");
    let mut decoder = OpusAudioDecoder::new().expect("Opus decoder init failed");
    let mut encoded_buf = [0u8; 512];
    let mut decoded_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];

    let mut opus_enc_times_us = Vec::with_capacity(iterations);
    let mut opus_dec_times_us = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let t0 = Instant::now();
        let enc_bytes = encoder.encode_float(&test_samples, &mut encoded_buf).unwrap();
        opus_enc_times_us.push(t0.elapsed().as_micros());

        let t1 = Instant::now();
        let _ = decoder.decode_float(Some(&encoded_buf[..enc_bytes]), &mut decoded_buf).unwrap();
        opus_dec_times_us.push(t1.elapsed().as_micros());
    }

    let avg_opus_enc_us = opus_enc_times_us.iter().sum::<u128>() as f64 / iterations as f64;
    let avg_opus_dec_us = opus_dec_times_us.iter().sum::<u128>() as f64 / iterations as f64;

    // 2. Raw PCM Encoder & Decoder Benchmark
    let mut pcm_bytes = vec![0u8; OPUS_FRAME_SIZE_SAMPLES * 2];
    let mut pcm_enc_times_us = Vec::with_capacity(iterations);
    let mut pcm_dec_times_us = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let t0 = Instant::now();
        let _ = encode_f32_to_pcm_i16_le(&test_samples, &mut pcm_bytes);
        pcm_enc_times_us.push(t0.elapsed().as_micros());

        let t1 = Instant::now();
        let _ = decode_pcm_i16_le_to_f32(&pcm_bytes, &mut decoded_buf);
        pcm_dec_times_us.push(t1.elapsed().as_micros());
    }

    let avg_pcm_enc_us = pcm_enc_times_us.iter().sum::<u128>() as f64 / iterations as f64;
    let avg_pcm_dec_us = pcm_dec_times_us.iter().sum::<u128>() as f64 / iterations as f64;

    // 3. Protocol Header Encode & Decode Benchmark
    let header = PacketHeader::new(PayloadType::Opus, 42, 12345678, 64);
    let mut header_buf = [0u8; HEADER_SIZE];
    let mut header_times_us = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let t0 = Instant::now();
        header.encode(&mut header_buf).unwrap();
        let _ = PacketHeader::decode(&header_buf).unwrap();
        header_times_us.push(t0.elapsed().as_micros());
    }

    let avg_header_us = header_times_us.iter().sum::<u128>() as f64 / iterations as f64;

    // 4. Dynamic Drift Resampler Benchmark
    let mut resampler =
        DynamicDriftResampler::new(OPUS_FRAME_SIZE_SAMPLES, 1.0005).expect("Resampler init failed");
    let mut resample_times_us = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let t0 = Instant::now();
        let _ = resampler.process(&test_samples).unwrap();
        resample_times_us.push(t0.elapsed().as_micros());
    }

    let avg_resample_us = resample_times_us.iter().sum::<u128>() as f64 / iterations as f64;

    // 5. Adaptive Jitter Buffer Benchmark
    let mut jb = AdaptiveJitterBuffer::new(DEFAULT_TARGET_WATERMARK_SAMPLES);
    let mut jb_times_us = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let t0 = Instant::now();
        jb.push_samples(&test_samples);
        let _ = jb.pop_samples(&mut decoded_buf);
        jb_times_us.push(t0.elapsed().as_micros());
    }

    let avg_jb_us = jb_times_us.iter().sum::<u128>() as f64 / iterations as f64;

    println!("================ LATENCY BENCHMARK REPORT ================");
    println!("Opus Encode:           avg = {:.2} us ({:.4} ms)", avg_opus_enc_us, avg_opus_enc_us / 1000.0);
    println!("Opus Decode:           avg = {:.2} us ({:.4} ms)", avg_opus_dec_us, avg_opus_dec_us / 1000.0);
    println!("PCM Encode:            avg = {:.2} us ({:.4} ms)", avg_pcm_enc_us, avg_pcm_enc_us / 1000.0);
    println!("PCM Decode:            avg = {:.2} us ({:.4} ms)", avg_pcm_dec_us, avg_pcm_dec_us / 1000.0);
    println!("Header Encode/Decode:  avg = {:.2} us ({:.4} ms)", avg_header_us, avg_header_us / 1000.0);
    println!("Rubato Resampler:      avg = {:.2} us ({:.4} ms)", avg_resample_us, avg_resample_us / 1000.0);
    println!("Jitter Buffer Push/Pop avg = {:.2} us ({:.4} ms)", avg_jb_us, avg_jb_us / 1000.0);
    println!("==========================================================");

    // Assertions ensuring processing runs well within budgets (frame deadline = 5000 us)
    assert!(
        avg_opus_enc_us < 1500.0,
        "Opus encode must take < 1.5ms per 5ms frame (got {:.2} us)",
        avg_opus_enc_us
    );
    assert!(
        avg_opus_dec_us < 1000.0,
        "Opus decode must take < 1.0ms per 5ms frame (got {:.2} us)",
        avg_opus_dec_us
    );
    assert!(
        avg_pcm_enc_us < 100.0,
        "Raw PCM encode must take < 0.1ms per frame (got {:.2} us)",
        avg_pcm_enc_us
    );
    assert!(
        avg_resample_us < 500.0,
        "Rubato resampling must take < 0.5ms per frame (got {:.2} us)",
        avg_resample_us
    );
    assert!(
        avg_header_us < 50.0,
        "Header encode/decode must take < 0.05ms (got {:.2} us)",
        avg_header_us
    );
}

#[tokio::test]
async fn test_end_to_end_opus_latency_budget_verification() {
    let test_port = 48460;
    let target_addr: SocketAddr = format!("127.0.0.1:{}", test_port).parse().unwrap();

    let receiver = UdpReceiver::bind(test_port).await.unwrap();
    let sender = UdpSender::bind(0, target_addr).await.unwrap();

    let mut encoder = OpusAudioEncoder::new().unwrap();
    let mut decoder = OpusAudioDecoder::new().unwrap();
    let mut resampler = DynamicDriftResampler::new(OPUS_FRAME_SIZE_SAMPLES, 1.0).unwrap();

    let test_samples = vec![0.5f32; OPUS_FRAME_SIZE_SAMPLES];
    let mut packet_buf = vec![0u8; 1024];
    let mut recv_buf = [0u8; 1024];
    let mut decoded_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];

    let mut total_processing_times_us = Vec::with_capacity(100);

    for seq in 1..=100 {
        let frame_start = Instant::now();

        // 1. Encode
        let enc_bytes = encoder.encode_float(&test_samples, &mut packet_buf[HEADER_SIZE..]).unwrap();

        // 2. Wrap packet
        let header = PacketHeader::new(PayloadType::Opus, seq, (seq as u64) * 5000, enc_bytes as u16);
        header.encode(&mut packet_buf[..HEADER_SIZE]).unwrap();

        // 3. UDP transit
        sender.send_packet(&packet_buf[..HEADER_SIZE + enc_bytes]).await.unwrap();
        let (_bytes, _) = receiver.recv_packet(&mut recv_buf).await.unwrap();

        // 4. Header decode
        let decoded_header = PacketHeader::decode(&recv_buf[..HEADER_SIZE]).unwrap();

        // 5. Audio decode
        let payload_len = decoded_header.payload_length as usize;
        let count = decoder
            .decode_float(Some(&recv_buf[HEADER_SIZE..HEADER_SIZE + payload_len]), &mut decoded_buf)
            .unwrap();
        assert_eq!(count, OPUS_FRAME_SIZE_SAMPLES);

        // 6. Resample
        let _ = resampler.process(&decoded_buf).unwrap();

        let processing_time = frame_start.elapsed().as_micros();
        total_processing_times_us.push(processing_time);
    }

    let avg_proc_ms = (total_processing_times_us.iter().sum::<u128>() as f64
        / total_processing_times_us.len() as f64)
        / 1000.0;

    // Total end-to-end latency budget breakdown:
    // Frame duration (5.0ms) + Jitter Buffer Target Watermark (5.0ms) + Active CPU & Network processing (~0.5ms)
    let total_end_to_end_latency_ms = FRAME_DURATION_MS + TARGET_WATERMARK_MS + avg_proc_ms as f32;

    println!(
        "[Opus Latency Budget] Frame Duration={:.1}ms, Watermark={:.1}ms, CPU+Net Transit={:.3}ms => TOTAL={:.3}ms (Target: <{:.1}ms)",
        FRAME_DURATION_MS, TARGET_WATERMARK_MS, avg_proc_ms, total_end_to_end_latency_ms, MAX_ALLOWED_OPUS_LATENCY_MS
    );

    assert!(
        total_end_to_end_latency_ms < MAX_ALLOWED_OPUS_LATENCY_MS,
        "Total Opus latency ({:.2}ms) must strictly adhere to sub-25ms target",
        total_end_to_end_latency_ms
    );
}

#[tokio::test]
async fn test_end_to_end_raw_pcm_latency_budget_verification() {
    let test_port = 48461;
    let target_addr: SocketAddr = format!("127.0.0.1:{}", test_port).parse().unwrap();

    let receiver = UdpReceiver::bind(test_port).await.unwrap();
    let sender = UdpSender::bind(0, target_addr).await.unwrap();

    let mut resampler = DynamicDriftResampler::new(OPUS_FRAME_SIZE_SAMPLES, 1.0).unwrap();

    let test_samples = vec![0.5f32; OPUS_FRAME_SIZE_SAMPLES];
    let mut packet_buf = vec![0u8; 1024];
    let mut recv_buf = [0u8; 1024];
    let mut decoded_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];

    let mut total_processing_times_us = Vec::with_capacity(100);

    for seq in 1..=100 {
        let frame_start = Instant::now();

        // 1. PCM Encode
        let pcm_len = encode_f32_to_pcm_i16_le(&test_samples, &mut packet_buf[HEADER_SIZE..]);

        // 2. Wrap packet
        let header = PacketHeader::new(PayloadType::RawPcm, seq, (seq as u64) * 5000, pcm_len as u16);
        header.encode(&mut packet_buf[..HEADER_SIZE]).unwrap();

        // 3. UDP transit
        sender.send_packet(&packet_buf[..HEADER_SIZE + pcm_len]).await.unwrap();
        let (_bytes, _) = receiver.recv_packet(&mut recv_buf).await.unwrap();

        // 4. Header decode
        let decoded_header = PacketHeader::decode(&recv_buf[..HEADER_SIZE]).unwrap();

        // 5. PCM Decode
        let payload_len = decoded_header.payload_length as usize;
        let count = decode_pcm_i16_le_to_f32(&recv_buf[HEADER_SIZE..HEADER_SIZE + payload_len], &mut decoded_buf);
        assert_eq!(count, OPUS_FRAME_SIZE_SAMPLES);

        // 6. Resample
        let _ = resampler.process(&decoded_buf).unwrap();

        let processing_time = frame_start.elapsed().as_micros();
        total_processing_times_us.push(processing_time);
    }

    let avg_proc_ms = (total_processing_times_us.iter().sum::<u128>() as f64
        / total_processing_times_us.len() as f64)
        / 1000.0;

    // Total end-to-end latency budget breakdown for Raw PCM:
    // Frame duration (5.0ms) + Jitter Buffer Target Watermark (5.0ms) + Active CPU & Network processing (~0.3ms)
    let total_end_to_end_latency_ms = FRAME_DURATION_MS + TARGET_WATERMARK_MS + avg_proc_ms as f32;

    println!(
        "[PCM Latency Budget] Frame Duration={:.1}ms, Watermark={:.1}ms, CPU+Net Transit={:.3}ms => TOTAL={:.3}ms (Target: <{:.1}ms)",
        FRAME_DURATION_MS, TARGET_WATERMARK_MS, avg_proc_ms, total_end_to_end_latency_ms, MAX_ALLOWED_PCM_LATENCY_MS
    );

    assert!(
        total_end_to_end_latency_ms < MAX_ALLOWED_PCM_LATENCY_MS,
        "Total Raw PCM latency ({:.2}ms) must strictly adhere to sub-16ms target",
        total_end_to_end_latency_ms
    );
}

#[tokio::test]
async fn test_live_frame_processing_deadline_adherence() {
    let test_port = 48462;
    let target_addr: SocketAddr = format!("127.0.0.1:{}", test_port).parse().unwrap();

    let receiver = UdpReceiver::bind(test_port).await.unwrap();
    let sender = UdpSender::bind(0, target_addr).await.unwrap();

    let mut encoder = OpusAudioEncoder::new().unwrap();
    let mut decoder = OpusAudioDecoder::new().unwrap();

    let test_samples = vec![0.7f32; OPUS_FRAME_SIZE_SAMPLES];
    let mut packet_buf = vec![0u8; 1024];
    let mut recv_buf = [0u8; 1024];
    let mut decoded_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];

    let frame_count = 300;
    let mut deadline_misses = 0;
    let mut max_processing_us = 0u128;

    for seq in 1..=frame_count {
        let frame_start = Instant::now();

        let enc_bytes = encoder.encode_float(&test_samples, &mut packet_buf[HEADER_SIZE..]).unwrap();
        let header = PacketHeader::new(PayloadType::Opus, seq, (seq as u64) * 5000, enc_bytes as u16);
        header.encode(&mut packet_buf[..HEADER_SIZE]).unwrap();

        sender.send_packet(&packet_buf[..HEADER_SIZE + enc_bytes]).await.unwrap();
        let _ = receiver.recv_packet(&mut recv_buf).await.unwrap();

        let count = decoder
            .decode_float(Some(&recv_buf[HEADER_SIZE..HEADER_SIZE + enc_bytes]), &mut decoded_buf)
            .unwrap();
        assert_eq!(count, OPUS_FRAME_SIZE_SAMPLES);

        let elapsed_us = frame_start.elapsed().as_micros();
        if elapsed_us > max_processing_us {
            max_processing_us = elapsed_us;
        }

        // Each 5ms audio frame must be processed within the 5.0ms (5000us) real-time audio deadline
        if elapsed_us > 5000 {
            deadline_misses += 1;
        }
    }

    println!(
        "[Frame Deadline Benchmark] Processed {} frames, Max frame processing time = {} us ({:.3} ms), Deadline misses = {}",
        frame_count, max_processing_us, max_processing_us as f64 / 1000.0, deadline_misses
    );

    assert_eq!(
        deadline_misses, 0,
        "Every 5ms frame must be processed well before the 5000us deadline to avoid audio underrun"
    );
    assert!(
        max_processing_us < 3000,
        "Max frame processing time should be < 3.0ms (got {:.3} ms)",
        max_processing_us as f64 / 1000.0
    );
}
