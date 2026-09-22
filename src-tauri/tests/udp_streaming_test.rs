use micstream_lib::audio::calculate_rms;
use micstream_lib::codec::{
    encode_f32_to_pcm_i16_le, OpusAudioDecoder, OpusAudioEncoder, OPUS_FRAME_SIZE_SAMPLES,
    OPUS_SAMPLE_RATE,
};
use micstream_lib::net::{StreamTelemetry, UdpReceiver, UdpSender};
use micstream_lib::protocol::{PacketHeader, PayloadType, HEADER_SIZE, MAGIC};
use micstream_lib::session::{AtomicF32, ClientSession, HostSession};
use micstream_lib::state::TransportMode;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_udp_streaming_opus_loopback() {
    let test_port = 48310;

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
    .expect("Failed to start host session for Opus test");

    let client_rms = Arc::new(AtomicF32::new(0.0));
    let client_telem = Arc::new(RwLock::new(StreamTelemetry::default()));

    let mut client = ClientSession::start(
        "127.0.0.1",
        test_port,
        TransportMode::Opus,
        Some("__TEST_MOCK__"),
        Arc::clone(&client_rms),
        Arc::clone(&client_telem),
        None,
        None,
    )
    .await
    .expect("Failed to start client session for Opus test");

    // Allow mock audio generation, transmission, decoding, and playback
    tokio::time::sleep(Duration::from_millis(450)).await;

    let c_rms = client_rms.get();
    let h_rms = host_rms.get();
    let telem = host_telem.read().await.clone();

    println!(
        "[E2E Opus] client_rms={:.4}, host_rms={:.4}, packets_received={}, packets_lost={}",
        c_rms, h_rms, telem.packets_sent, telem.packets_lost
    );

    assert!(c_rms > 0.1, "Client should generate audio with active RMS");
    assert!(h_rms > 0.1, "Host should decode and play audio with active RMS");
    assert!(
        telem.packets_sent >= 10,
        "Host should have received at least 10 audio packets, got {}",
        telem.packets_sent
    );
    assert_eq!(telem.packets_lost, 0, "No packets should be lost on loopback");

    client.stop().await;
    host.stop().await;

    assert_eq!(client_rms.get(), 0.0);
    assert_eq!(host_rms.get(), 0.0);
}

#[tokio::test]
async fn test_udp_streaming_raw_pcm_loopback() {
    let test_port = 48311;

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
    .expect("Failed to start host session for PCM test");

    let client_rms = Arc::new(AtomicF32::new(0.0));
    let client_telem = Arc::new(RwLock::new(StreamTelemetry::default()));

    let mut client = ClientSession::start(
        "127.0.0.1",
        test_port,
        TransportMode::RawPcm,
        Some("__TEST_MOCK__"),
        Arc::clone(&client_rms),
        Arc::clone(&client_telem),
        None,
        None,
    )
    .await
    .expect("Failed to start client session for PCM test");

    tokio::time::sleep(Duration::from_millis(450)).await;

    let c_rms = client_rms.get();
    let h_rms = host_rms.get();
    let telem = host_telem.read().await.clone();

    println!(
        "[E2E PCM] client_rms={:.4}, host_rms={:.4}, packets_received={}, packets_lost={}",
        c_rms, h_rms, telem.packets_sent, telem.packets_lost
    );

    assert!(c_rms > 0.1, "Client should generate audio with active RMS");
    assert!(h_rms > 0.1, "Host should decode and play audio with active RMS");
    assert!(
        telem.packets_sent >= 10,
        "Host should receive at least 10 audio packets"
    );
    assert_eq!(telem.packets_lost, 0, "Zero loss expected on local loopback");

    client.stop().await;
    host.stop().await;

    assert_eq!(client_rms.get(), 0.0);
    assert_eq!(host_rms.get(), 0.0);
}

#[tokio::test]
async fn test_udp_direct_datagram_transmission_roundtrip() {
    let test_port = 48312;
    let target_addr: SocketAddr = format!("127.0.0.1:{}", test_port).parse().unwrap();

    let receiver = UdpReceiver::bind(test_port)
        .await
        .expect("Failed to bind UDP receiver");
    let sender = UdpSender::bind(0, target_addr)
        .await
        .expect("Failed to bind UDP sender");

    let mut encoder = OpusAudioEncoder::new().expect("Opus encoder init failed");
    let mut decoder = OpusAudioDecoder::new().expect("Opus decoder init failed");

    let test_samples: Vec<f32> = (0..OPUS_FRAME_SIZE_SAMPLES)
        .map(|i| {
            let t = i as f32 / OPUS_SAMPLE_RATE as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7
        })
        .collect();

    let packet_count = 50;
    for seq in 1..=packet_count {
        let mut opus_buf = [0u8; 512];
        let enc_bytes = encoder
            .encode_float(&test_samples, &mut opus_buf)
            .expect("Opus encode failed");

        let header = PacketHeader::new(PayloadType::Opus, seq, (seq as u64) * 5000, enc_bytes as u16);
        let mut packet = vec![0u8; HEADER_SIZE + enc_bytes];
        header.encode(&mut packet[..HEADER_SIZE]).unwrap();
        packet[HEADER_SIZE..].copy_from_slice(&opus_buf[..enc_bytes]);

        sender.send_packet(&packet).await.expect("UDP send failed");

        let mut recv_buf = [0u8; 1024];
        let (bytes, src) = receiver
            .recv_packet(&mut recv_buf)
            .await
            .expect("UDP recv failed");

        assert_eq!(bytes, HEADER_SIZE + enc_bytes);
        assert_eq!(src.ip(), target_addr.ip());

        let decoded_header =
            PacketHeader::decode(&recv_buf[..HEADER_SIZE]).expect("Header decode failed");
        assert_eq!(decoded_header.magic, MAGIC);
        assert_eq!(decoded_header.sequence_number, seq);
        assert_eq!(decoded_header.payload_type, PayloadType::Opus);
        assert_eq!(decoded_header.payload_length as usize, enc_bytes);

        let mut decoded_audio = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
        let decoded_count = decoder
            .decode_float(Some(&recv_buf[HEADER_SIZE..bytes]), &mut decoded_audio)
            .expect("Opus decode failed");
        assert_eq!(decoded_count, OPUS_FRAME_SIZE_SAMPLES);
        assert!(calculate_rms(&decoded_audio) > 0.1);
    }
}

#[tokio::test]
async fn test_udp_streaming_watermark_dynamics() {
    for (port, watermark_ms) in [(48313, 2.5), (48314, 10.0)] {
        let host_rms = Arc::new(AtomicF32::new(0.0));
        let host_telem = Arc::new(RwLock::new(StreamTelemetry::default()));

        let mut host = HostSession::start(
            port,
            Some("__TEST_MOCK__"),
            watermark_ms,
            Arc::clone(&host_rms),
            Arc::clone(&host_telem),
            None,
            None,
        )
        .await
        .expect("Failed to start host session");

        let client_rms = Arc::new(AtomicF32::new(0.0));
        let client_telem = Arc::new(RwLock::new(StreamTelemetry::default()));

        let mut client = ClientSession::start(
            "127.0.0.1",
            port,
            TransportMode::Opus,
            Some("__TEST_MOCK__"),
            Arc::clone(&client_rms),
            Arc::clone(&client_telem),
            None,
            None,
        )
        .await
        .expect("Failed to start client session");

        tokio::time::sleep(Duration::from_millis(350)).await;

        let h_rms = host_rms.get();
        let telem = host_telem.read().await.clone();

        println!(
            "[Watermark {}ms] host_rms={:.4}, packets={}",
            watermark_ms, h_rms, telem.packets_sent
        );

        assert!(
            h_rms > 0.1,
            "Host playback should succeed with watermark {}ms",
            watermark_ms
        );
        assert!(
            telem.packets_sent > 5,
            "Packets should be processed at watermark {}ms",
            watermark_ms
        );

        client.stop().await;
        host.stop().await;
    }
}

#[tokio::test]
async fn test_udp_corrupted_packet_rejection() {
    let test_port = 48315;
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

    let raw_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind raw socket");

    // 1. Truncated packet (less than HEADER_SIZE 20 bytes)
    let truncated = [0x4Du8, 0x53, 0x01, 0x00, 0x00];
    let _ = raw_socket.send_to(&truncated, target_addr).await;

    // 2. Invalid magic bytes
    let mut invalid_magic = [0u8; 24];
    invalid_magic[0] = 0xDE;
    invalid_magic[1] = 0xAD;
    let _ = raw_socket.send_to(&invalid_magic, target_addr).await;

    // 3. Truncated payload: claims 100 bytes payload, but only 20 bytes sent in total
    let mut fake_header = [0u8; 20];
    fake_header[0] = 0x4D;
    fake_header[1] = 0x53;
    fake_header[2] = 0x01; // Version
    fake_header[3] = 0x00; // PayloadType Opus
    fake_header[16..18].copy_from_slice(&100u16.to_be_bytes()); // Claims 100 bytes
    let _ = raw_socket.send_to(&fake_header, target_addr).await;

    // Host should ignore all invalid packets and remain healthy
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        host_rms.get(),
        0.0,
        "Corrupted packets should not cause audio output"
    );

    // 4. Send valid PCM frames to verify receiver is fully functional
    let samples = vec![0.5f32; OPUS_FRAME_SIZE_SAMPLES];
    let mut pcm_bytes = vec![0u8; OPUS_FRAME_SIZE_SAMPLES * 2];
    encode_f32_to_pcm_i16_le(&samples, &mut pcm_bytes);

    let mut peak_rms = 0.0f32;
    for seq in 1..=40 {
        let header = PacketHeader::new(
            PayloadType::RawPcm,
            seq,
            (seq as u64) * 5000,
            pcm_bytes.len() as u16,
        );
        let mut packet = vec![0u8; HEADER_SIZE + pcm_bytes.len()];
        header.encode(&mut packet[..HEADER_SIZE]).unwrap();
        packet[HEADER_SIZE..].copy_from_slice(&pcm_bytes);

        let _ = raw_socket.send_to(&packet, target_addr).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        let r = host_rms.get();
        if r > peak_rms {
            peak_rms = r;
        }
    }

    let telem = host_telem.read().await.clone();
    println!(
        "[Corrupted Packet Resilience] Post-recovery peak_rms={:.4}, packets_received={}",
        peak_rms, telem.packets_sent
    );
    assert!(
        peak_rms > 0.1,
        "Host should recover immediately and play valid frames"
    );
    assert!(
        telem.packets_sent >= 35,
        "Host should have processed valid incoming packets"
    );

    host.stop().await;
}
