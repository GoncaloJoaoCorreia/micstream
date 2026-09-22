# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-02-18

### Added
- Ultra-low latency LAN microphone streaming utility for Moonlight & Apollo game streaming.
- Dual transport modes: Opus low-delay codec (5ms frames) and Raw PCM over UDP.
- Non-exclusive CPAL shared audio capture for Windows (WASAPI) and macOS (CoreAudio).
- Adaptive jitter buffer with dynamic watermark monitoring and rubato sinc drift compensation.
- Zero-configuration local network discovery via mDNS (`_micstream._udp`).
- Tauri v2 and React/TypeScript desktop GUI with real-time VU visualizers and latency telemetry.
