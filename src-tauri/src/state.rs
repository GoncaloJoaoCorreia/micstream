use crate::audio::AudioCaptureEngine;
use crate::net::{MdnsAdvertiser, MdnsBrowser, StreamTelemetry};
use crate::session::{AtomicF32, ClientSession, HostSession, StreamStatus};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::RwLock;

pub use crate::session::StreamStatus as AppStreamStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportMode {
    Opus,
    RawPcm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppRole {
    Client,
    Host,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub role: AppRole,
    pub transport_mode: TransportMode,
    pub target_jitter_ms: f32,
    pub udp_port: u16,
    pub selected_input_device: Option<String>,
    pub selected_output_device: Option<String>,
    pub manual_host_ip: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            role: AppRole::Client,
            transport_mode: TransportMode::Opus,
            target_jitter_ms: 5.0,
            udp_port: 48124,
            selected_input_device: None,
            selected_output_device: None,
            manual_host_ip: None,
        }
    }
}

pub struct InputMonitor {
    pub engine: AudioCaptureEngine,
    pub stop_tx: tokio::sync::oneshot::Sender<()>,
}

pub struct AppState {
    pub config: RwLock<AppConfig>,
    pub is_streaming: Arc<AtomicBool>,
    pub status: Arc<RwLock<StreamStatus>>,
    pub atomic_input_rms: Arc<AtomicF32>,
    pub atomic_output_rms: Arc<AtomicF32>,
    pub telemetry: Arc<RwLock<StreamTelemetry>>,
    pub active_client_session: Arc<tokio::sync::Mutex<Option<ClientSession>>>,
    pub active_host_session: Arc<tokio::sync::Mutex<Option<HostSession>>>,
    pub advertiser: Arc<tokio::sync::Mutex<Option<MdnsAdvertiser>>>,
    pub discovery_browser: Arc<tokio::sync::Mutex<Option<MdnsBrowser>>>,
    pub input_monitor: Arc<tokio::sync::Mutex<Option<InputMonitor>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            config: RwLock::new(AppConfig::default()),
            is_streaming: Arc::new(AtomicBool::new(false)),
            status: Arc::new(RwLock::new(StreamStatus::Idle)),
            atomic_input_rms: Arc::new(AtomicF32::new(0.0)),
            atomic_output_rms: Arc::new(AtomicF32::new(0.0)),
            telemetry: Arc::new(RwLock::new(StreamTelemetry::default())),
            active_client_session: Arc::new(tokio::sync::Mutex::new(None)),
            active_host_session: Arc::new(tokio::sync::Mutex::new(None)),
            advertiser: Arc::new(tokio::sync::Mutex::new(None)),
            discovery_browser: Arc::new(tokio::sync::Mutex::new(None)),
            input_monitor: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    pub fn is_active(&self) -> bool {
        self.is_streaming.load(Ordering::Relaxed)
    }

    pub fn set_active(&self, active: bool) {
        self.is_streaming.store(active, Ordering::Relaxed);
    }

    pub async fn get_status(&self) -> StreamStatus {
        *self.status.read().await
    }

    pub async fn set_status(&self, new_status: StreamStatus) {
        self.set_status_and_emit(new_status, None).await;
    }

    pub async fn set_status_and_emit(
        &self,
        new_status: StreamStatus,
        app: Option<&tauri::AppHandle>,
    ) {
        *self.status.write().await = new_status;
        let is_stream = matches!(
            new_status,
            StreamStatus::Streaming | StreamStatus::Listening
        );
        self.set_active(is_stream);
        if let Some(app) = app {
            let _ = app.emit("stream-status", new_status);
        }
    }

    pub async fn start_input_monitor(
        &self,
        device_name: Option<String>,
        app: Option<tauri::AppHandle>,
    ) -> Result<(), String> {
        let status = self.get_status().await;
        if status == StreamStatus::Streaming || status == StreamStatus::Listening {
            return Ok(());
        }

        self.stop_input_monitor(None).await;

        let atomic_rms = Arc::clone(&self.atomic_input_rms);
        let capture_engine = AudioCaptureEngine::start(device_name.as_deref(), move |_samples, rms| {
            atomic_rms.set(rms);
        })
        .map_err(|e| format!("Failed to start microphone monitor: {}", e))?;

        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();

        if let Some(app_handle) = app {
            let input_rms = Arc::clone(&self.atomic_input_rms);
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(std::time::Duration::from_millis(16));
                let mut peak: f32 = 0.0;
                let decay_rate: f32 = 0.95;

                loop {
                    tokio::select! {
                        _ = &mut stop_rx => {
                            let _ = app_handle.emit(
                                "audio-level",
                                serde_json::json!({
                                    "input_level": 0.0,
                                    "output_level": 0.0,
                                    "peak": 0.0
                                }),
                            );
                            break;
                        }
                        _ = ticker.tick() => {
                            let in_lvl = input_rms.get();
                            peak = (peak * decay_rate).max(in_lvl);

                            let _ = app_handle.emit(
                                "audio-level",
                                serde_json::json!({
                                    "input_level": in_lvl,
                                    "output_level": 0.0,
                                    "peak": peak
                                }),
                            );
                        }
                    }
                }
            });
        }

        let mut monitor_lock = self.input_monitor.lock().await;
        *monitor_lock = Some(InputMonitor {
            engine: capture_engine,
            stop_tx,
        });

        Ok(())
    }

    pub async fn stop_input_monitor(&self, app: Option<&tauri::AppHandle>) {
        let mut monitor_lock = self.input_monitor.lock().await;
        if let Some(monitor) = monitor_lock.take() {
            let _ = monitor.stop_tx.send(());
            monitor.engine.stop();
        }
        self.atomic_input_rms.set(0.0);
        if let Some(app) = app {
            let _ = app.emit(
                "audio-level",
                serde_json::json!({
                    "input_level": 0.0,
                    "output_level": 0.0,
                    "peak": 0.0
                }),
            );
        }
    }

    pub async fn stop_all(&self) {
        self.stop_all_and_emit(None).await;
    }

    pub async fn stop_all_and_emit(&self, app: Option<&tauri::AppHandle>) {
        self.stop_input_monitor(app).await;
        let mut client_lock = self.active_client_session.lock().await;
        if let Some(mut client) = client_lock.take() {
            client.stop().await;
        }
        let mut host_lock = self.active_host_session.lock().await;
        if let Some(mut host) = host_lock.take() {
            host.stop().await;
        }
        let mut adv_lock = self.advertiser.lock().await;
        if let Some(adv) = adv_lock.take() {
            adv.stop();
        }
        let mut browser_lock = self.discovery_browser.lock().await;
        if let Some(browser) = browser_lock.take() {
            browser.stop();
        }
        self.set_status_and_emit(StreamStatus::Idle, app).await;
    }
}
