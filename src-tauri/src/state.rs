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

    pub async fn stop_all(&self) {
        self.stop_all_and_emit(None).await;
    }

    pub async fn stop_all_and_emit(&self, app: Option<&tauri::AppHandle>) {
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
