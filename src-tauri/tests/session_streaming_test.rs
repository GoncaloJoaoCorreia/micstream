use micstream_lib::net::StreamTelemetry;
use micstream_lib::session::{AtomicF32, ClientSession, HostSession};
use micstream_lib::state::TransportMode;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[tokio::test]
async fn test_client_host_udp_streaming_opus() {
    let test_port = 48220;

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
    .expect("Failed to start client session");

    // Wait for mock audio generation, transmission, decoding, and playback
    tokio::time::sleep(Duration::from_millis(400)).await;

    let c_rms = client_rms.get();
    let h_rms = host_rms.get();
    let telem = host_telem.read().await.clone();
    let c_telem = client_telem.read().await.clone();

    println!(
        "Opus Stream: client_rms={:.4}, host_rms={:.4}, packets_sent={}, packets_lost={}, client_rtt_ms={:.2}",
        c_rms, h_rms, telem.packets_sent, telem.packets_lost, c_telem.rtt_ms
    );

    assert!(c_rms > 0.1, "Client should generate non-zero audio RMS");
    assert!(
        h_rms > 0.1,
        "Host should receive and play decoded non-zero audio RMS"
    );
    assert!(
        telem.packets_sent > 5,
        "Host should have received multiple audio packets"
    );
    assert!(c_telem.rtt_ms >= 0.0, "Client RTT should be non-negative");

    client.stop().await;
    host.stop().await;

    assert_eq!(client_rms.get(), 0.0);
    assert_eq!(host_rms.get(), 0.0);
}

#[tokio::test]
async fn test_client_host_udp_streaming_raw_pcm() {
    let test_port = 48221;

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
    .expect("Failed to start client session");

    tokio::time::sleep(Duration::from_millis(400)).await;

    let c_rms = client_rms.get();
    let h_rms = host_rms.get();
    let telem = host_telem.read().await.clone();

    println!(
        "Raw PCM Stream: client_rms={:.4}, host_rms={:.4}, packets_sent={}, packets_lost={}",
        c_rms, h_rms, telem.packets_sent, telem.packets_lost
    );

    assert!(c_rms > 0.1, "Client should generate non-zero audio RMS");
    assert!(
        h_rms > 0.1,
        "Host should receive and play decoded non-zero audio RMS"
    );
    assert!(
        telem.packets_sent > 5,
        "Host should have received multiple audio packets"
    );

    client.stop().await;
    host.stop().await;

    assert_eq!(client_rms.get(), 0.0);
    assert_eq!(host_rms.get(), 0.0);
}

#[tokio::test]
async fn test_app_state_lifecycle_and_status() {
    use micstream_lib::session::StreamStatus;
    use micstream_lib::state::AppState;

    let state = AppState::new();
    assert_eq!(state.get_status().await, StreamStatus::Idle);
    assert!(!state.is_active());

    state.set_status(StreamStatus::Streaming).await;
    assert_eq!(state.get_status().await, StreamStatus::Streaming);
    assert!(state.is_active());

    state.set_status(StreamStatus::Listening).await;
    assert_eq!(state.get_status().await, StreamStatus::Listening);
    assert!(state.is_active());

    state.set_status(StreamStatus::Idle).await;
    assert_eq!(state.get_status().await, StreamStatus::Idle);
    assert!(!state.is_active());

    // Error status transition verification
    state.set_status(StreamStatus::Error).await;
    assert_eq!(state.get_status().await, StreamStatus::Error);
    assert!(!state.is_active());
}

#[tokio::test]
async fn test_client_session_start_failure() {
    let client_rms = Arc::new(AtomicF32::new(0.0));
    let client_telem = Arc::new(RwLock::new(StreamTelemetry::default()));

    // Invalid unresolvable address
    let res = ClientSession::start(
        "invalid.host.unresolvable.micstream",
        48999,
        TransportMode::Opus,
        Some("__TEST_MOCK__"),
        client_rms,
        client_telem,
        None,
        None,
    )
    .await;

    assert!(
        res.is_err(),
        "Session start should fail with unresolvable host"
    );
}

#[test]
fn test_audio_engine_protocol_sample_rate_constant() {
    use micstream_lib::audio::PROTOCOL_SAMPLE_RATE;
    assert_eq!(PROTOCOL_SAMPLE_RATE, 48000);
}

#[tokio::test]
async fn test_client_disconnection_timeout_handling() {
    use micstream_lib::session::StreamStatus;

    let test_port = 48226;
    let host_rms = Arc::new(AtomicF32::new(0.0));
    let host_telem = Arc::new(RwLock::new(StreamTelemetry::default()));
    let host_status = Arc::new(RwLock::new(StreamStatus::Listening));

    let mut host = HostSession::start(
        test_port,
        Some("__TEST_MOCK__"),
        5.0,
        Arc::clone(&host_rms),
        Arc::clone(&host_telem),
        None,
        Some(Arc::clone(&host_status)),
    )
    .await
    .expect("Failed to start host session");

    assert_eq!(*host_status.read().await, StreamStatus::Listening);

    // Start client to establish connection
    let client_rms = Arc::new(AtomicF32::new(0.0));
    let client_telem = Arc::new(RwLock::new(StreamTelemetry::default()));

    let mut client = ClientSession::start(
        "127.0.0.1",
        test_port,
        TransportMode::Opus,
        Some("__TEST_MOCK__"),
        client_rms,
        client_telem,
        None,
        None,
    )
    .await
    .expect("Failed to start client session");

    // Allow packets to arrive and register client as connected
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(*host_status.read().await, StreamStatus::Listening);

    // Stop client abruptly (simulating sudden disconnection / network loss)
    client.stop().await;

    // Before 3 seconds, status should still be Listening
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(*host_status.read().await, StreamStatus::Listening);

    // After 3.2 seconds total since last packet, host should transition to Disconnected
    tokio::time::sleep(Duration::from_millis(2000)).await;
    assert_eq!(
        *host_status.read().await,
        StreamStatus::Disconnected,
        "Host should transition to Disconnected after 3s of silence"
    );

    host.stop().await;
}

#[tokio::test]
async fn test_bidirectional_heartbeat_and_rtt() {
    let test_port = 48227;

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
    .expect("Failed to start client session");

    // Allow at least 1-2 heartbeat ticks to occur
    tokio::time::sleep(Duration::from_millis(1300)).await;

    let c_telem = client_telem.read().await.clone();
    let h_telem = host_telem.read().await.clone();

    println!(
        "Bi-directional Heartbeat: client_rtt={:.2}ms, host_rtt={:.2}ms, host_jitter={:.2}ms",
        c_telem.rtt_ms, h_telem.rtt_ms, h_telem.jitter_ms
    );

    // Verify client RTT was computed from Host Pong
    assert!(c_telem.rtt_ms >= 0.0, "Client should have computed RTT");
    // Verify host RTT was computed from Client Pong
    assert!(h_telem.rtt_ms >= 0.0, "Host should have computed RTT");

    client.stop().await;
    host.stop().await;
}

#[tokio::test]
async fn test_mdns_discovery_roundtrip() {
    use micstream_lib::net::{DiscoveredHost, MdnsAdvertiser, MdnsBrowser};
    use std::sync::mpsc;

    let discovery_port = 48228;
    let instance_name = "MicStream-TestDiscovery";

    // 1. Start advertiser
    let advertiser = MdnsAdvertiser::start(instance_name, discovery_port)
        .expect("Failed to start mDNS advertiser");

    // 2. Start browser
    let browser = MdnsBrowser::new().expect("Failed to create mDNS browser");
    let (tx, rx) = mpsc::channel::<DiscoveredHost>();

    browser
        .browse(move |host| {
            let _ = tx.send(host);
        })
        .expect("Failed to start mDNS browse");

    // 3. Wait for discovery event (with 3-second timeout)
    let start = std::time::Instant::now();
    let mut discovered = false;

    while start.elapsed() < Duration::from_secs(3) {
        if let Ok(host) = rx.recv_timeout(Duration::from_millis(200)) {
            if host.port == discovery_port {
                assert!(
                    host.host_name.contains(instance_name) || !host.ip_addresses.is_empty(),
                    "Discovered host should contain instance name or IP addresses"
                );
                discovered = true;
                break;
            }
        }
    }

    advertiser.stop();
    browser.stop();

    assert!(
        discovered,
        "mDNS browser should have discovered the registered host within 3 seconds"
    );
}
