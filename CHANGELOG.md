# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-09-29

### Added
- Added macOS Gatekeeper quarantine handling and security bypass instructions to user installation and release documentation.

### Fixed
- Fixed macOS release application bundling and code signing by enabling ad-hoc signing (`signingIdentity: "-"`) to properly seal bundle resources and resolve Gatekeeper damaged binary errors.
- Updated macOS bundle identifier to `com.micstream.desktop` to prevent bundle extension conflict warnings.

## [0.1.1] - 2026-09-29

### Added
- Live input microphone monitoring in client mode while idle, allowing users to verify mic levels before streaming.
- `NSMicrophoneUsageDescription` in `Info.plist` for macOS microphone permission handling.
- Multi-format audio support in CPAL capture and playback engines (`F64`, `I32`, `I8`, `U16`, `U8`, `U32`, `I64`, `U64`) with automatic linear resampling to 48 kHz.
- Dedicated user installation guide (`docs/INSTALL.md`) and CI/CD release workflow documentation (`docs/RELEASE.md`).
- Project guidance and operational context for contributors and AI agents (`AGENTS.md`).

### Changed
- Simplified `README.md` to focus on user-facing benefits and streamlined quickstart steps.
- Renamed and organized `docs/ARCHITECTURE.md` as the authoritative system specification.
- Dedicated the project to the public domain under the Unlicense.

### Fixed
- Fixed audio sample downmixing and linear resampling when capturing from non-48 kHz audio input hardware.
- Fixed stream lifecycle management and socket binding safety when toggling between client and host modes.

## [0.1.0] - 2025-02-18

### Added
- Ultra-low latency LAN microphone streaming utility for Moonlight & Apollo game streaming.
- Dual transport modes: Opus low-delay codec (5ms frames) and Raw PCM over UDP.
- Non-exclusive CPAL shared audio capture for Windows (WASAPI) and macOS (CoreAudio).
- Adaptive jitter buffer with dynamic watermark monitoring and rubato sinc drift compensation.
- Zero-configuration local network discovery via mDNS (`_micstream._udp`).
- Tauri v2 and React/TypeScript desktop GUI with real-time VU visualizers and latency telemetry.
