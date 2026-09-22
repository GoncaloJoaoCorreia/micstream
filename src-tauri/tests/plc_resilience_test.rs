use micstream_lib::audio::calculate_rms;
use micstream_lib::codec::{
    OpusAudioDecoder, OpusAudioEncoder, OPUS_FRAME_SIZE_SAMPLES, OPUS_SAMPLE_RATE,
};
use micstream_lib::net::{StreamTelemetry, UdpSender};
use micstream_lib::protocol::{PacketHeader, PayloadType, HEADER_SIZE};
use micstream_lib::session::{AtomicF32, HostSession};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[test]
fn test_opus_packet_loss_concealment_direct() {
    let mut encoder = OpusAudioEncoder::new().expect("Encoder init failed");
    let mut decoder = OpusAudioDecoder::new().expect("Decoder init failed");

    // 1. Generate 440 Hz reference sine wave
    let mut original_samples = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    for (i, s) in original_samples.iter_mut().enumerate() {
        let t = i as f32 / OPUS_SAMPLE_RATE as f32;
        *s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7;
    }

    let mut encoded_packet = vec![0u8; 512];
    let enc_bytes = encoder
        .encode_float(&original_samples, &mut encoded_packet)
        .expect("Opus encode failed");

    // 2. Decode the first frame normally
    let mut frame1_decoded = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    let count1 = decoder
        .decode_float(Some(&encoded_packet[..enc_bytes]), &mut frame1_decoded)
        .expect("Decode frame 1 failed");
    assert_eq!(count1, OPUS_FRAME_SIZE_SAMPLES);
    let rms1 = calculate_rms(&frame1_decoded);
    assert!(rms1 > 0.1, "Frame 1 RMS should be active");

    // 3. Simulate missing packet: call decode_float(None, ...) for Packet Loss Concealment (PLC)
    let mut plc_frame1 = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    let count_plc1 = decoder
        .decode_float(None, &mut plc_frame1)
        .expect("Opus PLC decode 1 failed");
    assert_eq!(count_plc1, OPUS_FRAME_SIZE_SAMPLES);

    // Verify samples are non-NaN, finite, and have audio energy
    for s in &plc_frame1 {
        assert!(s.is_finite(), "PLC sample must be finite");
        assert!(!s.is_nan(), "PLC sample must not be NaN");
    }
    let rms_plc1 = calculate_rms(&plc_frame1);
    println!(
        "[Direct PLC] Frame 1 RMS={:.4}, PLC 1 RMS={:.4}",
        rms1, rms_plc1
    );
    assert!(
        rms_plc1 > 0.05,
        "PLC frame 1 should preserve audio energy from previous frame"
    );

    // 4. Consecutive PLC frames should smoothly decay in amplitude according to Opus PLC specification
    let mut plc_frame2 = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    let _ = decoder.decode_float(None, &mut plc_frame2);
    let rms_plc2 = calculate_rms(&plc_frame2);

    let mut plc_frame3 = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    let _ = decoder.decode_float(None, &mut plc_frame3);
    let rms_plc3 = calculate_rms(&plc_frame3);

    println!(
        "[Direct PLC Decay] PLC 1={:.4}, PLC 2={:.4}, PLC 3={:.4}",
        rms_plc1, rms_plc2, rms_plc3
    );
    assert!(
        rms_plc2 <= rms_plc1 + 0.01,
        "PLC 2 RMS should decay or maintain level relative to PLC 1"
    );
    assert!(
        rms_plc3 <= rms_plc2 + 0.01,
        "PLC 3 RMS should decay relative to PLC 2"
    );

    // 5. Subsequent valid packet decode recovers cleanly
    let mut recovered_frame = vec![0.0f32; OPUS_FRAME_SIZE_SAMPLES];
    let count_rec = decoder
        .decode_float(Some(&encoded_packet[..enc_bytes]), &mut recovered_frame)
        .expect("Decode post-PLC frame failed");
    assert_eq!(count_rec, OPUS_FRAME_SIZE_SAMPLES);
    let rms_rec = calculate_rms(&recovered_frame);
    assert!(
        rms_rec > 0.1,
        "Decoder should recover active signal after PLC"
    );
}

#[tokio::test]
async fn test_udp_streaming_packet_loss_concealment_5_percent() {
    let test_port = 48360;
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

    let mut encoder = OpusAudioEncoder::new().expect("Opus encoder init failed");

    // Generate test audio
    let test_samples: Vec<f32> = (0..OPUS_FRAME_SIZE_SAMPLES)
        .map(|i| {
            let t = i as f32 / OPUS_SAMPLE_RATE as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7
        })
        .collect();

    let total_packets = 100u32;
    // Drop 5% of packets: drop seq 20, 40, 60, 80, 95 (5 packets)
    let dropped_seqs = [20u32, 40, 60, 80, 95];
    let mut sent_count = 0;

    let mut peak_rms = 0.0f32;

    for seq in 1..=total_packets {
        if dropped_seqs.contains(&seq) {
            // Intentionally skip sending to simulate network drop
            tokio::time::sleep(Duration::from_millis(5)).await;
            continue;
        }

        let mut opus_buf = [0u8; 512];
        let enc_bytes = encoder
            .encode_float(&test_samples, &mut opus_buf)
            .expect("Opus encode failed");

        let header = PacketHeader::new(
            PayloadType::Opus,
            seq,
            (seq as u64) * 5000,
            enc_bytes as u16,
        );
        let mut packet = vec![0u8; HEADER_SIZE + enc_bytes];
        header.encode(&mut packet[..HEADER_SIZE]).unwrap();
        packet[HEADER_SIZE..].copy_from_slice(&opus_buf[..enc_bytes]);

        sender.send_packet(&packet).await.expect("UDP send failed");
        sent_count += 1;

        tokio::time::sleep(Duration::from_millis(5)).await;

        let r = host_rms.get();
        if r > peak_rms {
            peak_rms = r;
        }
    }

    tokio::time::sleep(Duration::from_millis(150)).await;

    let telem = host_telem.read().await.clone();
    println!(
        "[5% Drop Test] Sent={}, Telem: sent={}, lost={}, loss_pct={:.2}%, peak_rms={:.4}",
        sent_count, telem.packets_sent, telem.packets_lost, telem.packet_loss_percent, peak_rms
    );

    assert_eq!(
        telem.packets_lost, 5,
        "Host telemetry should record exactly 5 dropped packets"
    );
    assert!(
        (telem.packet_loss_percent - 5.0).abs() < 1.0,
        "Packet loss percentage should be approximately 5%"
    );
    assert!(
        peak_rms > 0.1,
        "Audio playback should maintain active audio through 5% packet loss"
    );

    host.stop().await;
}

#[tokio::test]
async fn test_udp_streaming_packet_loss_concealment_10_percent_burst() {
    let test_port = 48361;
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

    let mut encoder = OpusAudioEncoder::new().expect("Opus encoder init failed");

    let test_samples: Vec<f32> = (0..OPUS_FRAME_SIZE_SAMPLES)
        .map(|i| {
            let t = i as f32 / OPUS_SAMPLE_RATE as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7
        })
        .collect();

    let total_packets = 100u32;
    // Burst drops: 3 packets at 30..32, 3 packets at 60..62, 4 packets at 80..83 (total 10 packets)
    let burst_drops: Vec<u32> = vec![30, 31, 32, 60, 61, 62, 80, 81, 82, 83];
    let mut peak_rms = 0.0f32;

    for seq in 1..=total_packets {
        if burst_drops.contains(&seq) {
            tokio::time::sleep(Duration::from_millis(5)).await;
            continue;
        }

        let mut opus_buf = [0u8; 512];
        let enc_bytes = encoder
            .encode_float(&test_samples, &mut opus_buf)
            .expect("Opus encode failed");

        let header = PacketHeader::new(
            PayloadType::Opus,
            seq,
            (seq as u64) * 5000,
            enc_bytes as u16,
        );
        let mut packet = vec![0u8; HEADER_SIZE + enc_bytes];
        header.encode(&mut packet[..HEADER_SIZE]).unwrap();
        packet[HEADER_SIZE..].copy_from_slice(&opus_buf[..enc_bytes]);

        sender.send_packet(&packet).await.expect("UDP send failed");
        tokio::time::sleep(Duration::from_millis(5)).await;

        let r = host_rms.get();
        if r > peak_rms {
            peak_rms = r;
        }
    }

    tokio::time::sleep(Duration::from_millis(150)).await;

    let telem = host_telem.read().await.clone();
    println!(
        "[10% Burst Drop Test] Telem: sent={}, lost={}, loss_pct={:.2}%, peak_rms={:.4}",
        telem.packets_sent, telem.packets_lost, telem.packet_loss_percent, peak_rms
    );

    assert_eq!(
        telem.packets_lost, 10,
        "Host telemetry should record exactly 10 dropped burst packets"
    );
    assert!(
        (telem.packet_loss_percent - 10.0).abs() < 1.0,
        "Packet loss should be approximately 10%"
    );
    assert!(
        peak_rms > 0.1,
        "Audio playback should maintain continuous signal during burst drops"
    );

    host.stop().await;
}

#[tokio::test]
async fn test_severe_packet_loss_burst_boundary() {
    let test_port = 48362;
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

    let mut encoder = OpusAudioEncoder::new().expect("Opus encoder init failed");

    let test_samples: Vec<f32> = (0..OPUS_FRAME_SIZE_SAMPLES)
        .map(|i| {
            let t = i as f32 / OPUS_SAMPLE_RATE as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7
        })
        .collect();

    // Send packets 1..10
    for seq in 1..=10 {
        let mut opus_buf = [0u8; 512];
        let enc_bytes = encoder.encode_float(&test_samples, &mut opus_buf).unwrap();
        let header = PacketHeader::new(PayloadType::Opus, seq, (seq as u64) * 5000, enc_bytes as u16);
        let mut packet = vec![0u8; HEADER_SIZE + enc_bytes];
        header.encode(&mut packet[..HEADER_SIZE]).unwrap();
        packet[HEADER_SIZE..].copy_from_slice(&opus_buf[..enc_bytes]);
        sender.send_packet(&packet).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    // Drop packets 11..30 (20 packets lost = 100ms blackout, exceeding PLC limit of 10)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Resume stream from seq 31..60
    let mut post_blackout_peak = 0.0f32;
    for seq in 31..=60 {
        let mut opus_buf = [0u8; 512];
        let enc_bytes = encoder.encode_float(&test_samples, &mut opus_buf).unwrap();
        let header = PacketHeader::new(PayloadType::Opus, seq, (seq as u64) * 5000, enc_bytes as u16);
        let mut packet = vec![0u8; HEADER_SIZE + enc_bytes];
        header.encode(&mut packet[..HEADER_SIZE]).unwrap();
        packet[HEADER_SIZE..].copy_from_slice(&opus_buf[..enc_bytes]);
        sender.send_packet(&packet).await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;

        let r = host_rms.get();
        if r > post_blackout_peak {
            post_blackout_peak = r;
        }
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
    let telem = host_telem.read().await.clone();
    println!(
        "[Severe Burst Boundary] Sent total=40, Lost={}, post_blackout_peak={:.4}",
        telem.packets_lost, post_blackout_peak
    );

    assert_eq!(
        telem.packets_lost, 20,
        "Host should accurately detect 20 lost packets from blackout"
    );
    assert!(
        post_blackout_peak > 0.1,
        "Host should smoothly recover and resume active playback after blackout"
    );

    host.stop().await;
}
