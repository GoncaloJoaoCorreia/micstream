use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("Default input device not found")]
    DeviceNotFound,
    #[error("Specified device not found: {0}")]
    NamedDeviceNotFound(String),
    #[error("Devices enumeration error: {0}")]
    DevicesError(#[from] cpal::DevicesError),
    #[error("Failed to get supported input configs: {0}")]
    SupportedConfigError(#[from] cpal::SupportedStreamConfigsError),
    #[error("Failed to get default stream config: {0}")]
    DefaultStreamConfigError(#[from] cpal::DefaultStreamConfigError),
    #[error("Failed to build input stream: {0}")]
    BuildStreamError(#[from] cpal::BuildStreamError),
    #[error("Failed to play input stream: {0}")]
    PlayStreamError(#[from] cpal::PlayStreamError),
}

pub const PROTOCOL_SAMPLE_RATE: u32 = 48000;

pub struct AudioCaptureEngine {
    is_running: Arc<AtomicBool>,
    sample_rate: u32,
    stop_tx: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>,
    thread_handle: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl AudioCaptureEngine {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn start<F>(device_name: Option<&str>, mut sample_callback: F) -> Result<Self, CaptureError>
    where
        F: FnMut(&[f32], f32) + Send + 'static,
    {
        let device_name_owned = device_name.map(|s| s.to_string());
        let (init_tx, init_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel();
        let is_running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&is_running);

        let thread_handle = std::thread::spawn(move || {
            let build_res = (|| -> Result<(Stream, u32), CaptureError> {
                let host = cpal::default_host();
                let device = match device_name_owned.as_deref() {
                    Some(name) => host
                        .input_devices()?
                        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                        .ok_or_else(|| CaptureError::NamedDeviceNotFound(name.to_string()))?,
                    None => host
                        .default_input_device()
                        .ok_or(CaptureError::DeviceNotFound)?,
                };

                let (config, sample_format, negotiated_rate) = {
                    let mut selected = None;
                    if let Ok(configs) = device.supported_input_configs() {
                        let mut supported_48k = Vec::new();
                        for range in configs {
                            if range.min_sample_rate().0 <= PROTOCOL_SAMPLE_RATE
                                && range.max_sample_rate().0 >= PROTOCOL_SAMPLE_RATE
                            {
                                supported_48k.push(range);
                            }
                        }
                        if let Some(range) = supported_48k
                            .iter()
                            .find(|r| r.sample_format() == SampleFormat::F32)
                            .or_else(|| {
                                supported_48k
                                    .iter()
                                    .find(|r| r.sample_format() == SampleFormat::I16)
                            })
                            .or_else(|| supported_48k.first())
                        {
                            let conf =
                                range.with_sample_rate(cpal::SampleRate(PROTOCOL_SAMPLE_RATE));
                            let format = conf.sample_format();
                            selected = Some((conf.config(), format, PROTOCOL_SAMPLE_RATE));
                        }
                    }

                    if let Some(s) = selected {
                        s
                    } else {
                        let default_config = device.default_input_config()?;
                        let sample_rate = default_config.sample_rate().0;
                        let sample_format = default_config.sample_format();
                        if sample_rate != PROTOCOL_SAMPLE_RATE {
                            eprintln!(
                                "Notice: Input device does not natively support {} Hz (operating at {} Hz). MicStream Opus frames expect 48000 Hz / 240 samples.",
                                PROTOCOL_SAMPLE_RATE, sample_rate
                            );
                        }
                        (default_config.config(), sample_format, sample_rate)
                    }
                };

                let channels = config.channels;

                let err_fn = |err| {
                    eprintln!("CPAL input stream error: {:?}", err);
                };

                let stream = match sample_format {
                    SampleFormat::F32 => device.build_input_stream(
                        &config,
                        move |data: &[f32], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                return;
                            }
                            let mono_samples = downmix_to_mono_f32(data, channels as usize);
                            let rms = calculate_rms(&mono_samples);
                            sample_callback(&mono_samples, rms);
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::I16 => device.build_input_stream(
                        &config,
                        move |data: &[i16], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                return;
                            }
                            let f32_samples: Vec<f32> = data
                                .iter()
                                .map(|&s| {
                                    if s >= 0 {
                                        s as f32 / i16::MAX as f32
                                    } else {
                                        s as f32 / -(i16::MIN as f32)
                                    }
                                })
                                .collect();
                            let mono_samples = downmix_to_mono_f32(&f32_samples, channels as usize);
                            let rms = calculate_rms(&mono_samples);
                            sample_callback(&mono_samples, rms);
                        },
                        err_fn,
                        None,
                    )?,
                    _ => device.build_input_stream(
                        &config,
                        move |data: &[f32], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                return;
                            }
                            let mono_samples = downmix_to_mono_f32(data, channels as usize);
                            let rms = calculate_rms(&mono_samples);
                            sample_callback(&mono_samples, rms);
                        },
                        err_fn,
                        None,
                    )?,
                };

                stream.play()?;
                Ok((stream, negotiated_rate))
            })();

            match build_res {
                Ok((stream, rate)) => {
                    let _ = init_tx.send(Ok(rate));
                    let _ = stop_rx.recv();
                    drop(stream);
                }
                Err(e) => {
                    let _ = init_tx.send(Err(e));
                }
            }
        });

        match init_rx.recv() {
            Ok(Ok(sample_rate)) => Ok(Self {
                is_running,
                sample_rate,
                stop_tx: std::sync::Mutex::new(Some(stop_tx)),
                thread_handle: std::sync::Mutex::new(Some(thread_handle)),
            }),
            Ok(Err(e)) => {
                let _ = thread_handle.join();
                Err(e)
            }
            Err(_) => {
                let _ = thread_handle.join();
                Err(CaptureError::DeviceNotFound)
            }
        }
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
        if let Ok(mut lock) = self.stop_tx.lock() {
            if let Some(tx) = lock.take() {
                let _ = tx.send(());
            }
        }
        if let Ok(mut lock) = self.thread_handle.lock() {
            if let Some(handle) = lock.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for AudioCaptureEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn downmix_to_mono_f32(interleaved: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }

    let frame_count = interleaved.len() / channels;
    let mut mono = Vec::with_capacity(frame_count);
    for frame_idx in 0..frame_count {
        let mut sum = 0.0f32;
        for ch in 0..channels {
            sum += interleaved[frame_idx * channels + ch];
        }
        mono.push(sum / channels as f32);
    }
    mono
}

pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downmix_to_mono() {
        let stereo = vec![0.5f32, 0.5f32, -0.2f32, 0.4f32];
        let mono = downmix_to_mono_f32(&stereo, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.5f32).abs() < 0.0001);
        assert!((mono[1] - 0.1f32).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_rms() {
        let silence = vec![0.0f32; 100];
        assert_eq!(calculate_rms(&silence), 0.0);

        let constant = vec![0.5f32; 100];
        assert!((calculate_rms(&constant) - 0.5).abs() < 0.0001);
    }
}
