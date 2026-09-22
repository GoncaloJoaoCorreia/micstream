use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct StreamTelemetry {
    pub rtt_ms: f32,
    pub packet_loss_percent: f32,
    pub packets_sent: u64,
    pub packets_lost: u64,
    pub jitter_ms: f32,
}

pub struct HeartbeatTracker {
    last_ping_sent: Option<Instant>,
    rtt_ms: f32,
    packets_expected: u64,
    packets_received: u64,
    last_seq: Option<u32>,
    lost_count: u64,
    last_transit_time_us: Option<i64>,
    jitter_ms: f32,
}

impl Default for HeartbeatTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl HeartbeatTracker {
    pub fn new() -> Self {
        Self {
            last_ping_sent: None,
            rtt_ms: 0.0,
            packets_expected: 0,
            packets_received: 0,
            last_seq: None,
            lost_count: 0,
            last_transit_time_us: None,
            jitter_ms: 0.0,
        }
    }

    pub fn mark_ping_sent(&mut self) {
        self.last_ping_sent = Some(Instant::now());
    }

    pub fn mark_pong_received(&mut self) -> f32 {
        if let Some(sent) = self.last_ping_sent.take() {
            let elapsed = sent.elapsed().as_secs_f32() * 1000.0;
            self.update_rtt(elapsed);
        }
        self.rtt_ms
    }

    pub fn update_rtt(&mut self, sample_rtt_ms: f32) -> f32 {
        if self.rtt_ms == 0.0 {
            self.rtt_ms = sample_rtt_ms;
        } else {
            self.rtt_ms = 0.2 * sample_rtt_ms + 0.8 * self.rtt_ms;
        }
        self.rtt_ms
    }

    pub fn increment_packets_sent(&mut self) {
        self.packets_expected += 1;
    }

    pub fn record_sequence(&mut self, seq: u32) {
        if let Some(prev) = self.last_seq {
            if seq > prev + 1 {
                let lost = (seq - prev - 1) as u64;
                self.lost_count += lost;
                self.packets_expected += lost + 1;
            } else if seq > prev {
                self.packets_expected += 1;
            }
        } else {
            self.packets_expected = 1;
        }
        self.last_seq = Some(seq);
        self.packets_received += 1;
    }

    /// Records sequence number and calculates RFC 3550 interarrival jitter.
    pub fn record_packet(&mut self, seq: u32, timestamp_us: u64, arrival_us: u64) {
        self.record_sequence(seq);

        let transit = arrival_us as i64 - timestamp_us as i64;
        if let Some(prev_transit) = self.last_transit_time_us {
            let d = (transit - prev_transit).abs() as f32 / 1000.0; // transit variation in ms
            self.jitter_ms += (d - self.jitter_ms) / 16.0;
        }
        self.last_transit_time_us = Some(transit);
    }

    pub fn current_telemetry(&self) -> StreamTelemetry {
        let loss = if self.packets_expected > 0 {
            (self.lost_count as f32 / self.packets_expected as f32) * 100.0
        } else {
            0.0
        };

        StreamTelemetry {
            rtt_ms: self.rtt_ms,
            packet_loss_percent: loss,
            packets_sent: self.packets_expected.max(self.packets_received),
            packets_lost: self.lost_count,
            jitter_ms: self.jitter_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_sequence_loss_calculation() {
        let mut tracker = HeartbeatTracker::new();
        tracker.record_sequence(1);
        tracker.record_sequence(2);
        // packet 3 missing:
        tracker.record_sequence(4);

        let telemetry = tracker.current_telemetry();
        assert_eq!(telemetry.packets_lost, 1);
        assert_eq!(telemetry.packets_sent, 4);
        assert!((telemetry.packet_loss_percent - 25.0).abs() < 0.001);
    }

    #[test]
    fn test_heartbeat_jitter_calculation() {
        let mut tracker = HeartbeatTracker::new();
        // Packet 1: sent at 1000us, received at 2000us (transit = 1000us = 1ms)
        tracker.record_packet(1, 1000, 2000);
        assert_eq!(tracker.current_telemetry().jitter_ms, 0.0);

        // Packet 2: sent at 6000us (5ms later), received at 8000us (transit = 2000us = 2ms)
        // transit variation d = |2000 - 1000| = 1000us = 1.0ms
        // jitter = 0.0 + (1.0 - 0.0)/16.0 = 0.0625ms
        tracker.record_packet(2, 6000, 8000);
        let telem = tracker.current_telemetry();
        assert!(telem.jitter_ms > 0.0);
        assert!((telem.jitter_ms - 0.0625).abs() < 0.001);
    }

    #[test]
    fn test_heartbeat_rtt_update() {
        let mut tracker = HeartbeatTracker::new();
        tracker.update_rtt(10.0);
        assert_eq!(tracker.current_telemetry().rtt_ms, 10.0);

        // Exponential smoothing: 0.2 * 20.0 + 0.8 * 10.0 = 4.0 + 8.0 = 12.0
        tracker.update_rtt(20.0);
        assert_eq!(tracker.current_telemetry().rtt_ms, 12.0);
    }
}
