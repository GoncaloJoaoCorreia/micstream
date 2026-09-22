use crate::audio::{
    calculate_rms, AdaptiveJitterBuffer, AudioCaptureEngine, AudioPlaybackEngine,
    DynamicDriftResampler, MAX_WATERMARK_SAMPLES, MIN_WATERMARK_SAMPLES,
};
use crate::codec::{
    decode_pcm_i16_le_to_f32, encode_f32_to_pcm_i16_le, OpusAudioDecoder, OpusAudioEncoder,
    OPUS_FRAME_SIZE_SAMPLES,
};
use crate::net::{HeartbeatTracker, StreamTelemetry, UdpReceiver, UdpSender};
use crate::protocol::{PacketHeader, PayloadType, HEADER_SIZE};
use crate::state::TransportMode;
use byteorder::{BigEndian, ByteOrder};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::Emitter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StreamStatus {
    #[default]
    Idle,
    Streaming,
    Listening,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AudioLevelPayload {
    pub input_level: f32,
    pub output_level: f32,
    pub peak: f32,
}

#[derive(Debug, Default)]
pub struct AtomicF32 {
    bits: AtomicU32,
}

impl AtomicF32 {
    pub const fn new(val: f32) -> Self {
        Self {
            bits: AtomicU32::new(val.to_bits()),
        }
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.bits.load(Ordering::Relaxed))
    }

    pub fn set(&self, val: f32) {
        self.bits.store(val.to_bits(), Ordering::Relaxed);
    }
}

pub struct ClientSession {
    capture_engine: Option<AudioCaptureEngine>,
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
    atomic_rms: Arc<AtomicF32>,
    is_running: Arc<AtomicBool>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl ClientSession {
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        target_ip: &str,
        target_port: u16,
        mode: TransportMode,
        device_name: Option<&str>,
        atomic_rms: Arc<AtomicF32>,
        telemetry: Arc<tokio::sync::RwLock<StreamTelemetry>>,
        app_handle: Option<tauri::AppHandle>,
        _status_holder: Option<Arc<tokio::sync::RwLock<StreamStatus>>>,
    ) -> Result<Self, String> {
        let target_str = format!("{}:{}", target_ip, target_port);
        let mut addrs = tokio::net::lookup_host(&target_str)
            .await
            .map_err(|e| format!("Failed to resolve target address '{}': {}", target_str, e))?;
        let target_addr = addrs
            .next()
            .ok_or_else(|| format!("No IP address found for target '{}'", target_str))?;

        let sender = Arc::new(
            UdpSender::bind(0, target_addr)
                .await
                .map_err(|e| format!("Failed to bind UDP sender: {}", e))?,
        );

        let is_running = Arc::new(AtomicBool::new(true));
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        let ring_buffer = HeapRb::<f32>::new(48000);
        let (mut prod, mut cons) = ring_buffer.split();

        let capture_engine = if device_name == Some("__TEST_MOCK__") {
            let mock_running = Arc::clone(&is_running);
            let mock_rms = Arc::clone(&atomic_rms);
            tokio::spawn(async move {
                let mut sample_idx: usize = 0;
                while mock_running.load(Ordering::Relaxed) {
                    let mut chunk = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
                    for s in chunk.iter_mut() {
                        let t = sample_idx as f32 / 48000.0;
                        *s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7;
                        sample_idx = sample_idx.wrapping_add(1);
                    }
                    let rms = calculate_rms(&chunk);
                    mock_rms.set(rms);
                    let _ = prod.push_slice(&chunk);
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            });
            None
        } else {
            let rms_cb = Arc::clone(&atomic_rms);
            let engine = AudioCaptureEngine::start(device_name, move |samples, rms| {
                rms_cb.set(rms);
                let _ = prod.push_slice(samples);
            })
            .map_err(|e| format!("Failed to start audio capture: {}", e))?;
            Some(engine)
        };

        if let Some(ref engine) = capture_engine {
            if engine.sample_rate() != crate::audio::PROTOCOL_SAMPLE_RATE {
                eprintln!(
                    "Notice: Input device operates at {} Hz (protocol expects {} Hz). Audio frame timing assumes 48000 Hz.",
                    engine.sample_rate(),
                    crate::audio::PROTOCOL_SAMPLE_RATE
                );
            }
        }

        let mut opus_encoder = if mode == TransportMode::Opus {
            Some(OpusAudioEncoder::new().map_err(|e| format!("Opus encoder init failed: {}", e))?)
        } else {
            None
        };

        let session_start = Instant::now();
        let mut tasks = Vec::new();

        // 1. Audio sender worker task
        let sender_worker = Arc::clone(&sender);
        let telem_worker = Arc::clone(&telemetry);
        let running_worker = Arc::clone(&is_running);
        let sender_task = tokio::spawn(async move {
            let mut seq: u32 = 0;
            let start_time = session_start;
            let mut sample_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
            let mut payload_buf = [0u8; 1024];
            let mut datagram_buf = [0u8; HEADER_SIZE + 1024];

            while running_worker.load(Ordering::Relaxed) {
                if cons.occupied_len() >= OPUS_FRAME_SIZE_SAMPLES {
                    // Prevent latency buildup: if buffer has >100ms backlog, discard excess
                    while cons.occupied_len() > OPUS_FRAME_SIZE_SAMPLES * 8 {
                        let mut discard = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
                        cons.pop_slice(&mut discard);
                    }

                    let popped = cons.pop_slice(&mut sample_buf);
                    if popped == OPUS_FRAME_SIZE_SAMPLES {
                        let (ptype, payload_len) = match mode {
                            TransportMode::Opus => {
                                if let Some(ref mut enc) = opus_encoder {
                                    match enc.encode_float(&sample_buf, &mut payload_buf) {
                                        Ok(n) => (PayloadType::Opus, n),
                                        Err(_) => continue,
                                    }
                                } else {
                                    continue;
                                }
                            }
                            TransportMode::RawPcm => {
                                let n = encode_f32_to_pcm_i16_le(&sample_buf, &mut payload_buf);
                                (PayloadType::RawPcm, n)
                            }
                        };

                        let ts_us = start_time.elapsed().as_micros() as u64;
                        let header = PacketHeader::new(ptype, seq, ts_us, payload_len as u16);
                        if header.encode(&mut datagram_buf[..HEADER_SIZE]).is_ok() {
                            datagram_buf[HEADER_SIZE..HEADER_SIZE + payload_len]
                                .copy_from_slice(&payload_buf[..payload_len]);
                            let full_len = HEADER_SIZE + payload_len;
                            let _ = sender_worker.send_packet(&datagram_buf[..full_len]).await;
                            seq = seq.wrapping_add(1);
                            telem_worker.write().await.packets_sent += 1;
                        }
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        });
        tasks.push(sender_task);

        // 2. Heartbeat Ping task (1 Hz)
        let ping_sender = Arc::clone(&sender);
        let ping_running = Arc::clone(&is_running);
        let ping_task = tokio::spawn(async move {
            let mut ping_seq: u32 = 0;
            let start_time = session_start;
            let mut ping_buf = [0u8; HEADER_SIZE];

            while ping_running.load(Ordering::Relaxed) {
                let ts_us = start_time.elapsed().as_micros() as u64;
                let ping = PacketHeader::new(PayloadType::Ping, ping_seq, ts_us, 0);
                if ping.encode(&mut ping_buf).is_ok() {
                    let _ = ping_sender.send_packet(&ping_buf).await;
                    ping_seq = ping_seq.wrapping_add(1);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        tasks.push(ping_task);

        // 3. UDP socket receiver task (Pong / Ping listener)
        let sock_sender = Arc::clone(&sender);
        let sock_telem = Arc::clone(&telemetry);
        let sock_running = Arc::clone(&is_running);
        let sock_task = tokio::spawn(async move {
            let socket = sock_sender.socket();
            let mut recv_buf = [0u8; 512];
            let start_time = session_start;

            while sock_running.load(Ordering::Relaxed) {
                tokio::select! {
                    res = socket.recv_from(&mut recv_buf) => {
                        if let Ok((len, src)) = res {
                            if len >= HEADER_SIZE {
                                if let Ok(header) = PacketHeader::decode(&recv_buf[..HEADER_SIZE]) {
                                    match header.payload_type {
                                        PayloadType::Pong => {
                                            let now_us = start_time.elapsed().as_micros() as u64;
                                            if now_us >= header.timestamp_us {
                                                let rtt = (now_us - header.timestamp_us) as f32 / 1000.0;
                                                let mut telem = sock_telem.write().await;
                                                telem.rtt_ms = if telem.rtt_ms == 0.0 {
                                                    rtt
                                                } else {
                                                    0.2 * rtt + 0.8 * telem.rtt_ms
                                                };
                                                // If Host returned telemetry in Pong payload
                                                if header.payload_length >= 8 && len >= HEADER_SIZE + 8 {
                                                    telem.packet_loss_percent = BigEndian::read_f32(&recv_buf[HEADER_SIZE..HEADER_SIZE + 4]);
                                                    telem.jitter_ms = BigEndian::read_f32(&recv_buf[HEADER_SIZE + 4..HEADER_SIZE + 8]);
                                                    telem.packets_lost = ((telem.packet_loss_percent / 100.0) * telem.packets_sent as f32).round() as u64;
                                                }
                                            }
                                        }
                                        PayloadType::Ping => {
                                            let pong = PacketHeader::new(PayloadType::Pong, header.sequence_number, header.timestamp_us, 0);
                                            let mut pong_buf = [0u8; HEADER_SIZE];
                                            if pong.encode(&mut pong_buf).is_ok() {
                                                let _ = socket.send_to(&pong_buf, src).await;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {}
                }
            }
        });
        tasks.push(sock_task);

        // 4. Atomic Tick Bridge & Telemetry task (60 Hz audio-level, 1 Hz telemetry)
        let tick_rms = Arc::clone(&atomic_rms);
        let tick_telem = Arc::clone(&telemetry);
        let tick_running = Arc::clone(&is_running);
        let app_handle_clone = app_handle.clone();
        let ticker_task = tokio::spawn(async move {
            let mut peak: f32 = 0.0;
            let mut last_telemetry_emit = Instant::now();

            while tick_running.load(Ordering::Relaxed) {
                let input_level = tick_rms.get();
                peak = (peak * 0.95).max(input_level);

                if let Some(ref app) = app_handle_clone {
                    let payload = AudioLevelPayload {
                        input_level,
                        output_level: 0.0,
                        peak,
                    };
                    let _ = app.emit("audio-level", payload);

                    if last_telemetry_emit.elapsed() >= Duration::from_secs(1) {
                        let telem = tick_telem.read().await.clone();
                        let _ = app.emit("telemetry-update", telem);
                        last_telemetry_emit = Instant::now();
                    }
                }

                tokio::time::sleep(Duration::from_millis(16)).await;
            }

            // Emit final zero-level on shutdown
            if let Some(ref app) = app_handle_clone {
                let zero = AudioLevelPayload {
                    input_level: 0.0,
                    output_level: 0.0,
                    peak: 0.0,
                };
                let _ = app.emit("audio-level", zero);
            }
        });
        tasks.push(ticker_task);

        // Background monitor for cancellation signal
        let cancel_running = Arc::clone(&is_running);
        tokio::spawn(async move {
            let _ = cancel_rx.await;
            cancel_running.store(false, Ordering::Relaxed);
        });

        Ok(Self {
            capture_engine,
            cancel_tx: Some(cancel_tx),
            atomic_rms,
            is_running,
            tasks,
        })
    }

    pub async fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        if let Some(engine) = self.capture_engine.take() {
            engine.stop();
        }
        self.atomic_rms.set(0.0);
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

impl Drop for ClientSession {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        if let Some(ref engine) = self.capture_engine {
            engine.stop();
        }
        self.atomic_rms.set(0.0);
        for task in &self.tasks {
            task.abort();
        }
    }
}

pub struct HostSession {
    playback_engine: Option<AudioPlaybackEngine>,
    cancel_tx: Option<tokio::sync::oneshot::Sender<()>>,
    atomic_rms: Arc<AtomicF32>,
    is_running: Arc<AtomicBool>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl HostSession {
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        listen_port: u16,
        output_device_name: Option<&str>,
        target_jitter_ms: f32,
        atomic_rms: Arc<AtomicF32>,
        telemetry: Arc<tokio::sync::RwLock<StreamTelemetry>>,
        app_handle: Option<tauri::AppHandle>,
        status_holder: Option<Arc<tokio::sync::RwLock<StreamStatus>>>,
    ) -> Result<Self, String> {
        let receiver =
            Arc::new(UdpReceiver::bind(listen_port).await.map_err(|e| {
                format!("Failed to bind UDP receiver on port {}: {}", listen_port, e)
            })?);

        let is_running = Arc::new(AtomicBool::new(true));
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        let ring_buffer = HeapRb::<f32>::new(48000);
        let (mut play_prod, mut play_cons) = ring_buffer.split();

        let playback_engine = if output_device_name == Some("__TEST_MOCK__") {
            let mock_rms = Arc::clone(&atomic_rms);
            let mock_running = Arc::clone(&is_running);
            tokio::spawn(async move {
                let mut buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
                while mock_running.load(Ordering::Relaxed) {
                    let count = play_cons.pop_slice(&mut buf);
                    for s in buf.iter_mut().skip(count) {
                        *s = 0.0;
                    }
                    let rms = calculate_rms(&buf);
                    mock_rms.set(rms);
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            });
            None
        } else {
            let rms_cb = Arc::clone(&atomic_rms);
            let engine = AudioPlaybackEngine::start(output_device_name, move |mono_buf| {
                let count = play_cons.pop_slice(mono_buf);
                for s in mono_buf.iter_mut().skip(count) {
                    *s = 0.0;
                }
                let rms = calculate_rms(mono_buf);
                rms_cb.set(rms);
                rms
            })
            .map_err(|e| format!("Failed to start audio playback: {}", e))?;
            Some(engine)
        };

        if let Some(ref engine) = playback_engine {
            if engine.sample_rate() != crate::audio::PROTOCOL_SAMPLE_RATE {
                eprintln!(
                    "Notice: Output device operates at {} Hz (protocol expects {} Hz).",
                    engine.sample_rate(),
                    crate::audio::PROTOCOL_SAMPLE_RATE
                );
            }
        }

        let watermark_samples = ((target_jitter_ms * 48.0) as usize)
            .clamp(MIN_WATERMARK_SAMPLES, MAX_WATERMARK_SAMPLES);

        let session_start = Instant::now();
        let last_client_addr: Arc<tokio::sync::RwLock<Option<SocketAddr>>> =
            Arc::new(tokio::sync::RwLock::new(None));

        let mut tasks = Vec::new();

        // 1. Receiver & Decode worker task
        let recv_worker = Arc::clone(&receiver);
        let telem_worker = Arc::clone(&telemetry);
        let running_worker = Arc::clone(&is_running);
        let decode_client_addr = Arc::clone(&last_client_addr);
        let status_worker = status_holder.clone();
        let app_handle_worker = app_handle.clone();
        let decode_task = tokio::spawn(async move {
            let mut jitter_buffer = AdaptiveJitterBuffer::new(watermark_samples);
            let mut resampler = match DynamicDriftResampler::new(OPUS_FRAME_SIZE_SAMPLES, 1.0) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to initialize drift resampler: {:?}", e);
                    return;
                }
            };
            let mut opus_decoder = match OpusAudioDecoder::new() {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Failed to initialize Opus decoder: {:?}", e);
                    return;
                }
            };
            let mut tracker = HeartbeatTracker::new();
            let start_time = session_start;
            let mut last_seq: Option<u32> = None;
            let mut prebuffered = false;
            let mut recv_buf = [0u8; 2048];
            let mut last_packet_time: Option<Instant> = None;
            let mut client_connected = false;

            while running_worker.load(Ordering::Relaxed) {
                tokio::select! {
                    res = recv_worker.recv_packet(&mut recv_buf) => {
                        match res {
                            Ok((bytes, src)) => {
                                if bytes < HEADER_SIZE {
                                    continue;
                                }

                                let header = match PacketHeader::decode(&recv_buf[..HEADER_SIZE]) {
                                    Ok(h) => h,
                                    Err(_) => continue,
                                };

                                *decode_client_addr.write().await = Some(src);
                                last_packet_time = Some(Instant::now());
                                if !client_connected {
                                    client_connected = true;
                                    if let Some(ref sh) = status_worker {
                                        *sh.write().await = StreamStatus::Listening;
                                    }
                                    if let Some(ref app) = app_handle_worker {
                                        let _ = app.emit("stream-status", StreamStatus::Listening);
                                    }
                                }

                                match header.payload_type {
                                    PayloadType::Ping => {
                                        let current_telem = tracker.current_telemetry();
                                        let mut pong_buf = [0u8; HEADER_SIZE + 8];
                                        let pong = PacketHeader::new(
                                            PayloadType::Pong,
                                            header.sequence_number,
                                            header.timestamp_us,
                                            8,
                                        );
                                        if pong.encode(&mut pong_buf[..HEADER_SIZE]).is_ok() {
                                            BigEndian::write_f32(
                                                &mut pong_buf[HEADER_SIZE..HEADER_SIZE + 4],
                                                current_telem.packet_loss_percent,
                                            );
                                            BigEndian::write_f32(
                                                &mut pong_buf[HEADER_SIZE + 4..HEADER_SIZE + 8],
                                                current_telem.jitter_ms,
                                            );
                                            let _ = recv_worker.socket().send_to(&pong_buf, src).await;
                                        }
                                    }
                                    PayloadType::Pong => {
                                        let now_us = start_time.elapsed().as_micros() as u64;
                                        if now_us >= header.timestamp_us {
                                            let rtt = (now_us - header.timestamp_us) as f32 / 1000.0;
                                            tracker.update_rtt(rtt);
                                            *telem_worker.write().await = tracker.current_telemetry();
                                        }
                                    }
                                    PayloadType::Opus | PayloadType::RawPcm => {
                                        let payload_len = header.payload_length as usize;
                                        if bytes < HEADER_SIZE + payload_len {
                                            continue;
                                        }
                                        let payload = &recv_buf[HEADER_SIZE..HEADER_SIZE + payload_len];

                                        let now_us = start_time.elapsed().as_micros() as u64;
                                        tracker.record_packet(header.sequence_number, header.timestamp_us, now_us);

                                        // Sequence gap & PLC concealment
                                        if let Some(prev) = last_seq {
                                            if header.sequence_number > prev + 1 {
                                                let lost = (header.sequence_number - prev - 1) as usize;
                                                if lost <= 10 {
                                                    for _ in 0..lost {
                                                        let mut plc_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
                                                        if header.payload_type == PayloadType::Opus {
                                                            if let Ok(plc_samples) = opus_decoder.decode_float(None, &mut plc_buf) {
                                                                jitter_buffer.push_samples(&plc_buf[..plc_samples]);
                                                            }
                                                        } else {
                                                            jitter_buffer.push_samples(&plc_buf);
                                                        }

                                                        if prebuffered && jitter_buffer.current_occupancy() >= OPUS_FRAME_SIZE_SAMPLES {
                                                            let mut pop_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
                                                            jitter_buffer.pop_samples(&mut pop_buf);
                                                            let ratio = jitter_buffer.compute_drift_ratio();
                                                            let _ = resampler.set_ratio(ratio);
                                                            if let Ok(resampled) = resampler.process(&pop_buf) {
                                                                let _ = play_prod.push_slice(&resampled);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        last_seq = Some(header.sequence_number);

                                        // Decode incoming frame
                                        let mut decoded_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
                                        let decoded_count = match header.payload_type {
                                            PayloadType::Opus => {
                                                opus_decoder.decode_float(Some(payload), &mut decoded_buf).unwrap_or(0)
                                            }
                                            PayloadType::RawPcm => {
                                                decode_pcm_i16_le_to_f32(payload, &mut decoded_buf)
                                            }
                                            _ => 0,
                                        };

                                        if decoded_count > 0 {
                                            jitter_buffer.push_samples(&decoded_buf[..decoded_count]);
                                        }

                                        if !prebuffered && jitter_buffer.current_occupancy() >= watermark_samples {
                                            prebuffered = true;
                                        }

                                        if prebuffered {
                                            while jitter_buffer.current_occupancy() >= OPUS_FRAME_SIZE_SAMPLES {
                                                let mut pop_buf = [0.0f32; OPUS_FRAME_SIZE_SAMPLES];
                                                jitter_buffer.pop_samples(&mut pop_buf);
                                                let ratio = jitter_buffer.compute_drift_ratio();
                                                let _ = resampler.set_ratio(ratio);
                                                if let Ok(resampled) = resampler.process(&pop_buf) {
                                                    let _ = play_prod.push_slice(&resampled);
                                                }
                                            }
                                        }

                                        *telem_worker.write().await = tracker.current_telemetry();
                                    }
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        if client_connected {
                            if let Some(last_time) = last_packet_time {
                                if last_time.elapsed() >= Duration::from_secs(3) {
                                    client_connected = false;
                                    prebuffered = false;
                                    if let Some(ref sh) = status_worker {
                                        *sh.write().await = StreamStatus::Disconnected;
                                    }
                                    if let Some(ref app) = app_handle_worker {
                                        let _ = app.emit("stream-status", StreamStatus::Disconnected);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        tasks.push(decode_task);

        // 2. Host Ping task (1 Hz Ping to connected client)
        let ping_recv_worker = Arc::clone(&receiver);
        let ping_running = Arc::clone(&is_running);
        let ping_client_addr = Arc::clone(&last_client_addr);
        let ping_start = session_start;
        let host_ping_task = tokio::spawn(async move {
            let mut host_ping_seq: u32 = 0;
            let mut ping_buf = [0u8; HEADER_SIZE];
            while ping_running.load(Ordering::Relaxed) {
                if let Some(client_addr) = *ping_client_addr.read().await {
                    let ts_us = ping_start.elapsed().as_micros() as u64;
                    let ping = PacketHeader::new(PayloadType::Ping, host_ping_seq, ts_us, 0);
                    if ping.encode(&mut ping_buf).is_ok() {
                        let _ = ping_recv_worker
                            .socket()
                            .send_to(&ping_buf, client_addr)
                            .await;
                        host_ping_seq = host_ping_seq.wrapping_add(1);
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        tasks.push(host_ping_task);

        // 2. Atomic Tick Bridge & Telemetry task (60 Hz audio-level, 1 Hz telemetry)
        let tick_rms = Arc::clone(&atomic_rms);
        let tick_telem = Arc::clone(&telemetry);
        let tick_running = Arc::clone(&is_running);
        let app_handle_clone = app_handle.clone();
        let ticker_task = tokio::spawn(async move {
            let mut peak: f32 = 0.0;
            let mut last_telemetry_emit = Instant::now();

            while tick_running.load(Ordering::Relaxed) {
                let output_level = tick_rms.get();
                peak = (peak * 0.95).max(output_level);

                if let Some(ref app) = app_handle_clone {
                    let payload = AudioLevelPayload {
                        input_level: 0.0,
                        output_level,
                        peak,
                    };
                    let _ = app.emit("audio-level", payload);

                    if last_telemetry_emit.elapsed() >= Duration::from_secs(1) {
                        let telem = tick_telem.read().await.clone();
                        let _ = app.emit("telemetry-update", telem);
                        last_telemetry_emit = Instant::now();
                    }
                }

                tokio::time::sleep(Duration::from_millis(16)).await;
            }

            // Emit final zero-level on shutdown
            if let Some(ref app) = app_handle_clone {
                let zero = AudioLevelPayload {
                    input_level: 0.0,
                    output_level: 0.0,
                    peak: 0.0,
                };
                let _ = app.emit("audio-level", zero);
            }
        });
        tasks.push(ticker_task);

        // Background monitor for cancellation signal
        let cancel_running = Arc::clone(&is_running);
        tokio::spawn(async move {
            let _ = cancel_rx.await;
            cancel_running.store(false, Ordering::Relaxed);
        });

        Ok(Self {
            playback_engine,
            cancel_tx: Some(cancel_tx),
            atomic_rms,
            is_running,
            tasks,
        })
    }

    pub async fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        if let Some(engine) = self.playback_engine.take() {
            engine.stop();
        }
        self.atomic_rms.set(0.0);
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

impl Drop for HostSession {
    fn drop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
        if let Some(ref engine) = self.playback_engine {
            engine.stop();
        }
        self.atomic_rms.set(0.0);
        for task in &self.tasks {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_f32_operations() {
        let atomic = AtomicF32::new(0.5);
        assert_eq!(atomic.get(), 0.5);
        atomic.set(0.85);
        assert_eq!(atomic.get(), 0.85);
    }

    #[test]
    fn test_stream_status_serialization() {
        let status = StreamStatus::Streaming;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Streaming\"");
        let deserialized: StreamStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, StreamStatus::Streaming);
    }

    #[test]
    fn test_audio_level_payload_serialization() {
        let payload = AudioLevelPayload {
            input_level: 0.12,
            output_level: 0.34,
            peak: 0.56,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"input_level\":0.12"));
        assert!(json.contains("\"output_level\":0.34"));
        assert!(json.contains("\"peak\":0.56"));
    }
}
