use crate::audio::{
    check_host_virtual_driver_status, list_input_devices, list_output_devices, AudioDeviceInfo,
    VirtualDriverStatus,
};
use crate::net::{get_default_host_name, DiscoveredHost, MdnsAdvertiser, MdnsBrowser};
use crate::session::{ClientSession, HostSession, StreamStatus};
use crate::state::{AppState, TransportMode};
use std::sync::Arc;
use tauri::{Emitter, State};

#[tauri::command]
pub fn get_input_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    list_input_devices()
}

#[tauri::command]
pub fn get_output_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    list_output_devices()
}

#[tauri::command]
pub fn get_virtual_driver_status() -> VirtualDriverStatus {
    check_host_virtual_driver_status()
}

#[tauri::command]
pub async fn start_stream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    target_ip: String,
    target_port: u16,
    mode: String,
    device_name: Option<String>,
) -> Result<(), String> {
    let transport_mode = match mode.trim().to_lowercase().as_str() {
        "opus" => TransportMode::Opus,
        "raw_pcm" | "rawpcm" | "pcm" => TransportMode::RawPcm,
        other => {
            return Err(format!(
                "Unsupported transport mode '{}'. Supported modes: 'opus', 'pcm'",
                other
            ))
        }
    };

    // Cleanly tear down any active sessions and advertiser
    let mut adv_lock = state.advertiser.lock().await;
    if let Some(adv) = adv_lock.take() {
        adv.stop();
    }
    drop(adv_lock);

    let mut host_lock = state.active_host_session.lock().await;
    if let Some(mut existing_host) = host_lock.take() {
        existing_host.stop().await;
    }
    drop(host_lock);

    let mut client_lock = state.active_client_session.lock().await;
    if let Some(mut existing_client) = client_lock.take() {
        existing_client.stop().await;
    }

    let session = match ClientSession::start(
        &target_ip,
        target_port,
        transport_mode,
        device_name.as_deref(),
        Arc::clone(&state.atomic_input_rms),
        Arc::clone(&state.telemetry),
        Some(app.clone()),
        Some(Arc::clone(&state.status)),
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            state
                .set_status_and_emit(StreamStatus::Error, Some(&app))
                .await;
            return Err(e);
        }
    };

    *client_lock = Some(session);
    drop(client_lock);

    state
        .set_status_and_emit(StreamStatus::Streaming, Some(&app))
        .await;

    // Update state config
    {
        let mut cfg = state.config.write().await;
        cfg.role = crate::state::AppRole::Client;
        cfg.transport_mode = transport_mode;
        cfg.manual_host_ip = Some(target_ip);
        cfg.udp_port = target_port;
        if let Some(dev) = device_name {
            cfg.selected_input_device = Some(dev);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_stream(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut client_lock = state.active_client_session.lock().await;
    if let Some(mut session) = client_lock.take() {
        session.stop().await;
    }
    drop(client_lock);

    state
        .set_status_and_emit(StreamStatus::Idle, Some(&app))
        .await;
    Ok(())
}

#[tauri::command]
pub async fn start_listen(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    port: u16,
    output_device_name: Option<String>,
) -> Result<(), String> {
    // Cleanly tear down any active client session and discovery browser
    let mut browser_lock = state.discovery_browser.lock().await;
    if let Some(browser) = browser_lock.take() {
        browser.stop();
    }
    drop(browser_lock);

    let mut client_lock = state.active_client_session.lock().await;
    if let Some(mut existing_client) = client_lock.take() {
        existing_client.stop().await;
    }
    drop(client_lock);

    let mut host_lock = state.active_host_session.lock().await;
    if let Some(mut existing_host) = host_lock.take() {
        existing_host.stop().await;
    }

    let mut adv_lock = state.advertiser.lock().await;
    if let Some(adv) = adv_lock.take() {
        adv.stop();
    }

    let target_jitter = state.config.read().await.target_jitter_ms;

    let session = match HostSession::start(
        port,
        output_device_name.as_deref(),
        target_jitter,
        Arc::clone(&state.atomic_output_rms),
        Arc::clone(&state.telemetry),
        Some(app.clone()),
        Some(Arc::clone(&state.status)),
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            state
                .set_status_and_emit(StreamStatus::Error, Some(&app))
                .await;
            return Err(e);
        }
    };

    *host_lock = Some(session);
    drop(host_lock);

    // Broadcast host presence via mDNS
    let host_name = get_default_host_name();
    match MdnsAdvertiser::start(&host_name, port) {
        Ok(adv) => {
            *adv_lock = Some(adv);
        }
        Err(e) => {
            eprintln!("Warning: Failed to start mDNS advertiser: {}", e);
        }
    }
    drop(adv_lock);

    state
        .set_status_and_emit(StreamStatus::Listening, Some(&app))
        .await;

    // Update state config
    {
        let mut cfg = state.config.write().await;
        cfg.role = crate::state::AppRole::Host;
        cfg.udp_port = port;
        if let Some(dev) = output_device_name {
            cfg.selected_output_device = Some(dev);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_listen(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut host_lock = state.active_host_session.lock().await;
    if let Some(mut session) = host_lock.take() {
        session.stop().await;
    }
    drop(host_lock);

    let mut adv_lock = state.advertiser.lock().await;
    if let Some(adv) = adv_lock.take() {
        adv.stop();
    }
    drop(adv_lock);

    state
        .set_status_and_emit(StreamStatus::Idle, Some(&app))
        .await;
    Ok(())
}

#[tauri::command]
pub async fn get_stream_status(state: State<'_, AppState>) -> Result<StreamStatus, String> {
    Ok(state.get_status().await)
}

#[tauri::command]
pub async fn start_discovery(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut browser_lock = state.discovery_browser.lock().await;
    if let Some(existing) = browser_lock.take() {
        existing.stop();
    }

    let browser =
        MdnsBrowser::new().map_err(|e| format!("Failed to create mDNS browser: {}", e))?;
    let app_handle = app.clone();
    browser
        .browse(move |host: DiscoveredHost| {
            let _ = app_handle.emit("host-discovered", host);
        })
        .map_err(|e| format!("Failed to start mDNS browsing: {}", e))?;

    *browser_lock = Some(browser);
    Ok(())
}

#[tauri::command]
pub async fn stop_discovery(state: State<'_, AppState>) -> Result<(), String> {
    let mut browser_lock = state.discovery_browser.lock().await;
    if let Some(browser) = browser_lock.take() {
        browser.stop();
    }
    Ok(())
}

#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> Result<crate::state::AppConfig, String> {
    Ok(state.config.read().await.clone())
}

#[tauri::command]
pub async fn update_app_config(
    state: State<'_, AppState>,
    config: crate::state::AppConfig,
) -> Result<(), String> {
    let mut cfg = state.config.write().await;
    *cfg = config;
    Ok(())
}

#[tauri::command]
pub async fn set_target_jitter(state: State<'_, AppState>, jitter_ms: f32) -> Result<(), String> {
    if !(1.0..=100.0).contains(&jitter_ms) {
        return Err("Jitter buffer watermark must be between 1.0 and 100.0 ms".into());
    }
    let mut cfg = state.config.write().await;
    cfg.target_jitter_ms = jitter_ms;
    Ok(())
}
