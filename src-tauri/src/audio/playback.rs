use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlaybackError {
    #[error("Default output device not found")]
    DeviceNotFound,
    #[error("Specified device not found: {0}")]
    NamedDeviceNotFound(String),
    #[error("Devices enumeration error: {0}")]
    DevicesError(#[from] cpal::DevicesError),
    #[error("Failed to get supported output configs: {0}")]
    SupportedConfigError(#[from] cpal::SupportedStreamConfigsError),
    #[error("Failed to get default stream config: {0}")]
    DefaultStreamConfigError(#[from] cpal::DefaultStreamConfigError),
    #[error("Failed to build output stream: {0}")]
    BuildStreamError(#[from] cpal::BuildStreamError),
    #[error("Failed to play output stream: {0}")]
    PlayStreamError(#[from] cpal::PlayStreamError),
    #[error("Unsupported sample format: {0:?}")]
    UnsupportedSampleFormat(SampleFormat),
}

pub const PROTOCOL_SAMPLE_RATE: u32 = 48000;

pub struct AudioPlaybackEngine {
    is_running: Arc<AtomicBool>,
    sample_rate: u32,
    stop_tx: std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>,
    thread_handle: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl AudioPlaybackEngine {
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn start<F>(device_name: Option<&str>, mut sample_source: F) -> Result<Self, PlaybackError>
    where
        F: FnMut(&mut [f32]) -> f32 + Send + 'static, // fills mono buffer, returns RMS
    {
        let device_name_owned = device_name.map(|s| s.to_string());
        let (init_tx, init_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = std::sync::mpsc::channel();
        let is_running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&is_running);

        let thread_handle = std::thread::spawn(move || {
            let build_res = (|| -> Result<(Stream, u32), PlaybackError> {
                let host = cpal::default_host();
                let device = match device_name_owned.as_deref() {
                    Some(name) => host
                        .output_devices()?
                        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                        .ok_or_else(|| PlaybackError::NamedDeviceNotFound(name.to_string()))?,
                    None => host
                        .default_output_device()
                        .ok_or(PlaybackError::DeviceNotFound)?,
                };

                let (config, sample_format, negotiated_rate) = {
                    let mut selected = None;
                    if let Ok(configs) = device.supported_output_configs() {
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
                        let default_config = device.default_output_config()?;
                        let sample_rate = default_config.sample_rate().0;
                        let sample_format = default_config.sample_format();
                        if sample_rate != PROTOCOL_SAMPLE_RATE {
                            eprintln!(
                                "Notice: Output device does not natively support {} Hz (operating at {} Hz).",
                                PROTOCOL_SAMPLE_RATE, sample_rate
                            );
                        }
                        (default_config.config(), sample_format, sample_rate)
                    }
                };

                let channels = config.channels as usize;

                let err_fn = |err| {
                    eprintln!("CPAL output stream error: {:?}", err);
                };

                let mut mono_buf = Vec::new();

                let stream = match sample_format {
                    SampleFormat::F32 => device.build_output_stream(
                        &config,
                        move |data: &mut [f32], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(0.0);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx];
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::F64 => device.build_output_stream(
                        &config,
                        move |data: &mut [f64], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(0.0);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx] as f64;
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::I16 => device.build_output_stream(
                        &config,
                        move |data: &mut [i16], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(0);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx].clamp(-1.0, 1.0);
                                let sample_i16 = if sample >= 0.0 {
                                    (sample * i16::MAX as f32) as i16
                                } else {
                                    (sample * -(i16::MIN as f32)) as i16
                                };
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample_i16;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::I32 => device.build_output_stream(
                        &config,
                        move |data: &mut [i32], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(0);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx].clamp(-1.0, 1.0);
                                let sample_i32 = if sample >= 0.0 {
                                    (sample * i32::MAX as f32) as i32
                                } else {
                                    (sample * -(i32::MIN as f32)) as i32
                                };
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample_i32;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::I8 => device.build_output_stream(
                        &config,
                        move |data: &mut [i8], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(0);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx].clamp(-1.0, 1.0);
                                let sample_i8 = if sample >= 0.0 {
                                    (sample * i8::MAX as f32) as i8
                                } else {
                                    (sample * -(i8::MIN as f32)) as i8
                                };
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample_i8;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::U16 => device.build_output_stream(
                        &config,
                        move |data: &mut [u16], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(32768);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx].clamp(-1.0, 1.0);
                                let sample_u16 = ((sample * 32767.0) + 32768.0).clamp(0.0, 65535.0) as u16;
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample_u16;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::U8 => device.build_output_stream(
                        &config,
                        move |data: &mut [u8], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(128);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx].clamp(-1.0, 1.0);
                                let sample_u8 = ((sample * 127.0) + 128.0).clamp(0.0, 255.0) as u8;
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample_u8;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::U32 => device.build_output_stream(
                        &config,
                        move |data: &mut [u32], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(2147483648);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx].clamp(-1.0, 1.0);
                                let sample_u32 = ((sample as f64 * 2147483647.0) + 2147483648.0).clamp(0.0, 4294967295.0) as u32;
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample_u32;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::I64 => device.build_output_stream(
                        &config,
                        move |data: &mut [i64], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(0);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx].clamp(-1.0, 1.0);
                                let sample_i64 = (sample as f64 * i64::MAX as f64) as i64;
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample_i64;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    SampleFormat::U64 => device.build_output_stream(
                        &config,
                        move |data: &mut [u64], _| {
                            if !running_clone.load(Ordering::Relaxed) {
                                data.fill(9223372036854775808);
                                return;
                            }
                            let frames = data.len() / channels;
                            if mono_buf.len() < frames {
                                mono_buf.resize(frames, 0.0);
                            }
                            sample_source(&mut mono_buf[..frames]);
                            for frame_idx in 0..frames {
                                let sample = mono_buf[frame_idx].clamp(-1.0, 1.0);
                                let sample_u64 = ((sample as f64 * 9223372036854775807.0) + 9223372036854775808.0) as u64;
                                for ch in 0..channels {
                                    data[frame_idx * channels + ch] = sample_u64;
                                }
                            }
                        },
                        err_fn,
                        None,
                    )?,
                    format => {
                        return Err(PlaybackError::UnsupportedSampleFormat(format));
                    }
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
                Err(PlaybackError::DeviceNotFound)
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

impl Drop for AudioPlaybackEngine {
    fn drop(&mut self) {
        self.stop();
    }
}
