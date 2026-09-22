use micstream_lib::audio::{
    calculate_rms, AdaptiveJitterBuffer, DynamicDriftResampler, DEFAULT_TARGET_WATERMARK_SAMPLES,
};
use micstream_lib::codec::{
    encode_f32_to_pcm_i16_le, OPUS_FRAME_SIZE_SAMPLES, OPUS_SAMPLE_RATE,
};
use micstream_lib::net::{StreamTelemetry, UdpSender};
use micstream_lib::protocol::{PacketHeader, PayloadType, HEADER_SIZE};
use micstream_lib::session::{AtomicF32, HostSession};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[test]
fn test_clock_skew_simulation_fast_source_drift_ratio() {
    let target_watermark = DEFAULT_TARGET_WATERMARK_SAMPLES; // 240 samples = 5ms @ 48kHz
    let mut jb = AdaptiveJitterBuffer::new(target_watermark);

    // Initial occupancy EMA is target_watermark, initial ratio should be 1.0
    assert_eq!(jb.target_watermark(), target_watermark);
    assert!((jb.compute_drift_ratio() - 1.0).abs() < 1e-6);

    // Fast source produces extra samples over streaming: buffer accumulates 300 samples
    jb.push_samples(&vec![0.1f32; 300]);
    for _ in 0..20 {
        jb.push_samples(&[]); // settle EMA towards 300
    }

    let occupancy = jb.current_occupancy();
    let ema = jb.occupancy_ema();
    let ratio = jb.compute_drift_ratio();

    println!(
        "[Fast Clock Skew] Occupancy={}, EMA={:.2}, Drift Ratio={:.6}",
        occupancy, ema, ratio
    );

    assert_eq!(occupancy, 300);
    assert!(
        ema > target_watermark as f64,
        "Occupancy EMA should be higher than target watermark"
    );
    assert!(
        ratio > 1.0,
        "Drift ratio must be > 1.0 to speed up playback and drain excess samples"
    );
    assert!(
        ratio <= 1.001,
        "Drift ratio adjustment must be clamped within ±0.1% (<= 1.001)"
    );
}

#[test]
fn test_clock_skew_simulation_slow_source_drift_ratio() {
    let target_watermark = DEFAULT_TARGET_WATERMARK_SAMPLES;
    let mut jb = AdaptiveJitterBuffer::new(target_watermark);

    // Slow source results in deficit: buffer drops to 180 samples
    jb.push_samples(&vec![0.1f32; 180]);
    for _ in 0..20 {
        jb.push_samples(&[]); // settle EMA towards 180
    }

    let occupancy = jb.current_occupancy();
    let ema = jb.occupancy_ema();
    let ratio = jb.compute_drift_ratio();

    println!(
        "[Slow Clock Skew] Occupancy={}, EMA={:.2}, Drift Ratio={:.6}",
        occupancy, ema, ratio
    );

    assert_eq!(occupancy, 180);
    assert!(
        ema < target_watermark as f64,
        "Occupancy EMA should be lower than target watermark"
    );
    assert!(
        ratio < 1.0,
        "Drift ratio must be < 1.0 to slow down playback and allow buffer to fill"
    );
    assert!(
        ratio >= 0.999,
        "Drift ratio adjustment must be clamped within ±0.1% (>= 0.999)"
    );
}

#[test]
fn test_closed_loop_drift_adaptation_stabilization() {
    let target_watermark = 240usize;

    // 1. Test convergence from overfilled state (340 samples) back to target watermark (240)
    let mut jb_high = AdaptiveJitterBuffer::new(target_watermark);
    jb_high.push_samples(&vec![0.2f32; 340]);
    for _ in 0..30 {
        jb_high.push_samples(&[]); // settle EMA to 340
    }
    let initial_high_ratio = jb_high.compute_drift_ratio();
    println!("[High Skew Initial] Ratio={:.6}", initial_high_ratio);
    assert!(initial_high_ratio > 1.0, "Overfilled buffer must produce drift ratio > 1.0");

    // As resampler speeds up, net drainage occurs: drain 2 samples per step over 50 steps
    for _ in 0..50 {
        let mut out = [0.0f32; 2];
        jb_high.pop_samples(&mut out);
    }

    assert_eq!(jb_high.current_occupancy(), target_watermark);
    for _ in 0..30 {
        jb_high.push_samples(&[]); // settle EMA at watermark
    }
    let final_high_ratio = jb_high.compute_drift_ratio();
    println!("[High Skew Final] Ratio={:.6}", final_high_ratio);
    assert!(
        (final_high_ratio - 1.0).abs() < 0.0002,
        "Drift ratio should return to ~1.0 once buffer drains back to watermark"
    );

    // 2. Test convergence from underfilled state (140 samples) back to target watermark (240)
    let mut jb_low = AdaptiveJitterBuffer::new(target_watermark);
    jb_low.push_samples(&vec![0.2f32; 140]);
    for _ in 0..30 {
        jb_low.push_samples(&[]); // settle EMA to 140
    }
    let initial_low_ratio = jb_low.compute_drift_ratio();
    println!("[Low Skew Initial] Ratio={:.6}", initial_low_ratio);
    assert!(initial_low_ratio < 1.0, "Underfilled buffer must produce drift ratio < 1.0");

    // As resampler slows down, net replenishment occurs: add 2 samples per step over 50 steps
    for _ in 0..50 {
        jb_low.push_samples(&[0.2f32; 2]);
    }

    assert_eq!(jb_low.current_occupancy(), target_watermark);
    for _ in 0..30 {
        jb_low.push_samples(&[]); // settle EMA at watermark
    }
    let final_low_ratio = jb_low.compute_drift_ratio();
    println!("[Low Skew Final] Ratio={:.6}", final_low_ratio);
    assert!(
        (final_low_ratio - 1.0).abs() < 0.0002,
        "Drift ratio should return to ~1.0 once buffer replenishes back to watermark"
    );
}

#[test]
fn test_drift_resampler_audio_fidelity_at_skew_boundaries() {
    // Generate reference 440 Hz tone
    let mut original_samples = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    for (i, s) in original_samples.iter_mut().enumerate() {
        let t = i as f32 / OPUS_SAMPLE_RATE as f32;
        *s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7;
    }
    let input_rms = calculate_rms(&original_samples);

    // Test resampler at minimum skew ratio (0.999), unity (1.0), and maximum skew ratio (1.001)
    for ratio in [0.999, 1.0, 1.001] {
        let mut resampler =
            DynamicDriftResampler::new(OPUS_FRAME_SIZE_SAMPLES, ratio).expect("Init failed");

        let resampled = resampler
            .process(&original_samples)
            .expect("Process failed");

        assert!(
            !resampled.is_empty(),
            "Resampled output should not be empty"
        );

        // Check for NaN or Inf
        for s in &resampled {
            assert!(s.is_finite(), "Output sample must be finite at ratio {}", ratio);
            assert!(!s.is_nan(), "Output sample must not be NaN at ratio {}", ratio);
        }

        let output_rms = calculate_rms(&resampled);
        let rms_delta = (output_rms - input_rms).abs();
        println!(
            "[Drift Audio Fidelity] Ratio={:.4}, Input RMS={:.4}, Output RMS={:.4}, Delta={:.4}",
            ratio, input_rms, output_rms, rms_delta
        );

        // RMS should be preserved with less than 2% deviation
        assert!(
            rms_delta < 0.05,
            "Resampling at ratio {} should preserve signal RMS fidelity",
            ratio
        );
    }
}

#[tokio::test]
async fn test_udp_streaming_with_simulated_client_clock_skew() {
    let test_port = 48410;
    let target_addr: SocketAddr = format!("127.0.0.1:{}", test_port).parse().unwrap();

    let host_rms = Arc::new(AtomicF32::new(0.0));
    let host_telem = Arc::new(RwLock::new(StreamTelemetry::default()));

    let mut host = HostSession::start(
        test_port,
        Some("__TEST_MOCK__"),
        5.0,
        Arc::clone(&host_rms),
        Arc::clone(&host_telem),
        None,
        None,
    )
    .await
    .expect("Failed to start host session");

    let sender = UdpSender::bind(0, target_addr)
        .await
        .expect("Failed to bind UDP sender");

    let samples = vec![0.6f32; OPUS_FRAME_SIZE_SAMPLES];
    let mut pcm_bytes = vec![0u8; OPUS_FRAME_SIZE_SAMPLES * 2];
    encode_f32_to_pcm_i16_le(&samples, &mut pcm_bytes);

    // Send 100 frames with slight pacing skew (every 4.5ms instead of 5.0ms)
    // simulating a client sending packets slightly ahead of nominal time
    let mut peak_rms = 0.0f32;
    for seq in 1..=100 {
        let header = PacketHeader::new(
            PayloadType::RawPcm,
            seq,
            (seq as u64) * 5000,
            pcm_bytes.len() as u16,
        );
        let mut packet = vec![0u8; HEADER_SIZE + pcm_bytes.len()];
        header.encode(&mut packet[..HEADER_SIZE]).unwrap();
        packet[HEADER_SIZE..].copy_from_slice(&pcm_bytes);

        sender.send_packet(&packet).await.expect("UDP send failed");
        tokio::time::sleep(Duration::from_micros(4500)).await;

        let r = host_rms.get();
        if r > peak_rms {
            peak_rms = r;
        }
    }

    tokio::time::sleep(Duration::from_millis(100)).await;

    let telem = host_telem.read().await.clone();
    println!(
        "[Clock Skew UDP Stream] Received={}, Lost={}, Peak RMS={:.4}",
        telem.packets_sent, telem.packets_lost, peak_rms
    );

    assert!(
        telem.packets_sent >= 90,
        "Host should receive all skewed packets without dropping"
    );
    assert_eq!(telem.packets_lost, 0, "No packet loss expected under local skew");
    assert!(
        peak_rms > 0.1,
        "Playback engine should maintain active RMS output despite clock skew"
    );

    host.stop().await;
}
