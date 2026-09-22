use std::collections::VecDeque;

pub const DEFAULT_TARGET_WATERMARK_SAMPLES: usize = 240; // 5ms @ 48kHz
pub const MIN_WATERMARK_SAMPLES: usize = 120; // 2.5ms
pub const MAX_WATERMARK_SAMPLES: usize = 960; // 20ms

pub struct AdaptiveJitterBuffer {
    buffer: VecDeque<f32>,
    target_watermark: usize,
    occupancy_ema: f64,
    ema_alpha: f64,
}

impl AdaptiveJitterBuffer {
    pub fn new(target_watermark: usize) -> Self {
        let target = target_watermark.clamp(MIN_WATERMARK_SAMPLES, MAX_WATERMARK_SAMPLES);
        Self {
            buffer: VecDeque::with_capacity(target * 4),
            target_watermark: target,
            occupancy_ema: target as f64,
            ema_alpha: 0.05, // low-pass filter
        }
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        self.buffer.extend(samples.iter().copied());
        self.update_ema();
    }

    pub fn pop_samples(&mut self, out: &mut [f32]) -> usize {
        let count = out.len().min(self.buffer.len());
        for sample in out.iter_mut().take(count) {
            *sample = self.buffer.pop_front().unwrap_or(0.0);
        }
        for sample in out.iter_mut().skip(count) {
            *sample = 0.0;
        }
        self.update_ema();
        count
    }

    pub fn current_occupancy(&self) -> usize {
        self.buffer.len()
    }

    pub fn occupancy_ema(&self) -> f64 {
        self.occupancy_ema
    }

    pub fn target_watermark(&self) -> usize {
        self.target_watermark
    }

    /// Computes the recommended resampling ratio to steer occupancy towards target_watermark.
    /// If buffer has too many samples (client faster), ratio > 1.0 (resampler speeds up playback).
    /// If buffer has too few samples (client slower), ratio < 1.0 (resampler slows down playback).
    pub fn compute_drift_ratio(&self) -> f64 {
        let delta = self.occupancy_ema - self.target_watermark as f64;
        let adjustment = (delta * 0.00001).clamp(-0.001, 0.001);
        1.0 + adjustment
    }

    fn update_ema(&mut self) {
        let current = self.buffer.len() as f64;
        self.occupancy_ema = self.ema_alpha * current + (1.0 - self.ema_alpha) * self.occupancy_ema;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jitter_buffer_push_pop() {
        let mut jb = AdaptiveJitterBuffer::new(DEFAULT_TARGET_WATERMARK_SAMPLES);
        let samples = vec![0.1f32; 240];
        jb.push_samples(&samples);
        assert_eq!(jb.current_occupancy(), 240);

        let mut out = vec![0.0f32; 120];
        let popped = jb.pop_samples(&mut out);
        assert_eq!(popped, 120);
        assert_eq!(jb.current_occupancy(), 120);
        assert_eq!(out[0], 0.1f32);
    }

    #[test]
    fn test_drift_ratio_direction() {
        let mut jb = AdaptiveJitterBuffer::new(240);
        // Overfilled buffer -> client is producing too fast -> ratio should be > 1.0
        jb.push_samples(&vec![0.0f32; 480]);
        for _ in 0..10 {
            jb.update_ema();
        }
        assert!(jb.compute_drift_ratio() > 1.0);
    }
}
