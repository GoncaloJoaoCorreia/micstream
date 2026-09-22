# MicStream — Master Plan & Technical Specification

> **Ultra-Low Latency Cross-Platform LAN Microphone Streaming Utility**  
> *Targeted for Moonlight, Apollo, and Sunshine Game Streaming Environments*

---

## Table of Contents

1. [Executive Summary & Problem Statement](#1-executive-summary--problem-statement)
2. [Scope & Target Environment](#2-scope--target-environment)
3. [System Architecture & Data Flow](#3-system-architecture--data-flow)
4. [Audio Pipeline & Latency Budget](#4-audio-pipeline--latency-budget)
5. [Audio Capture & Virtual Playback Engine](#5-audio-capture--virtual-playback-engine)
6. [Dual-Mode Audio Codec Architecture](#6-dual-mode-audio-codec-architecture)
7. [Binary Network Transport Protocol](#7-binary-network-transport-protocol)
8. [Adaptive Jitter Buffering & Dynamic Drift Resampling](#8-adaptive-jitter-buffering--dynamic-drift-resampling)
9. [Zero-Configuration LAN Discovery (mDNS) & Handshake](#9-zero-configuration-lan-discovery-mdns--handshake)
10. [Desktop Application & UI/UX Design](#10-desktop-application--uiux-design)
11. [Project Directory & Module Structure](#11-project-directory--module-structure)
12. [Milestone Roadmap & Testable Deliverables](#12-milestone-roadmap--testable-deliverables)
13. [Testing, Benchmarking & Verification Strategy](#13-testing-benchmarking--verification-strategy)
14. [MVP Acceptance Criteria](#14-mvp-acceptance-criteria)

---

## 1. Executive Summary & Problem Statement

### 1.1 Context
Game streaming over local area networks (LAN) has become a mainstream solution for competitive and casual gamers using software such as **Moonlight**, **Apollo**, and **Sunshine**. In typical configurations, a high-performance Windows PC processes the game while streaming display and audio output to a lightweight portable client (e.g., an Apple Silicon MacBook or low-power laptop).

### 1.2 The Gap
While Moonlight and Sunshine excel at downstream video and audio delivery (host to client), upstream microphone streaming (client to host) is either unsupported, plagued by high latency, or locks the physical microphone exclusively on the client machine. Players who want to communicate in game proximity chat, Discord on the host, or multiplayer lobbies must either keep long cables connected to the host or deal with distracting audio latency.

### 1.3 The Solution: MicStream
**MicStream** provides a dedicated, ultra-low latency, one-way audio pipeline that captures microphone input from the client device and transmits it across the local network to the host machine. On the host, MicStream routes the incoming audio stream into a standard virtual audio loopback device (VB-Audio Virtual Cable on Windows, BlackHole on macOS). Applications running on the host system detect and utilize this stream as an ordinary hardware microphone input.

---

## 2. Scope & Target Environment

### 2.1 In Scope
- **Cross-Platform Matrix**:
  - Windows 10/11 (x64)
  - macOS (Apple Silicon ARM64, macOS 12 Monterey and newer)
  - Interoperable across any permutation (Mac-to-Windows, Windows-to-Windows, Windows-to-Mac, Mac-to-Mac).
- **Single Unified Binary**: The application includes both Sender (Client) and Receiver (Host) functionality within a single executable, switchable via a toggle in the UI.
- **Non-Exclusive Microphone Capture**: Operates via OS-native shared capture interfaces (`WASAPI Shared` on Windows, `CoreAudio` on macOS), allowing concurrent access by client-side applications (e.g., local Discord).
- **Multi-Device Selection**: Dynamically enumerates all physical and virtual audio inputs with support for hot-plugging.
- **Dual Transport Protocols**:
  - *Opus Low-Delay over UDP* (Default): 5ms frames, CELT mode, resilient against packet loss via PLC.
  - *Raw PCM over UDP* (Advanced): Uncompressed 48 kHz 16-bit audio for minimal processing latency.
- **Zero-Configuration Discovery**: Automated mDNS announcement and browsing with manual IP:port fallback and saved connection history.
- **Clock Drift Correction**: Bandlimited sinc micro-resampling (`rubato`) compensating for hardware crystal oscillator drift between client and host audio interfaces.
- **Modern Desktop UI**: Tauri v2 + React + Tailwind CSS with live VU metering and real-time network telemetry.

### 2.2 Out of Scope
- **Linux Platform Support**: Excluded for the MVP; focus is centered on Windows and Apple Silicon macOS.
- **Two-Way Audio Streaming**: Return system audio streaming from host to client is already handled by Moonlight/Sunshine.
- **Network Encryption / TLS**: Designed strictly for secure private LANs; unencrypted UDP transmission avoids encryption latency overhead.
- **Proprietary Kernel Driver Development**: Utilizes established, free virtual loopback drivers (VB-Cable / BlackHole) to prevent driver maintenance and kernel signing issues.

---

## 3. System Architecture & Data Flow

### 3.1 High-Level Architecture Diagram

```
+─────────────────────────────────────────────────────────────+
│                        CLIENT DEVICE                        │
│                 (macOS Apple Silicon / Windows)             │
│                                                             │
│   Physical Microphone (USB / Headset / Built-in)            │
│                         │                                   │
│                         ▼                                   │
│           CPAL Shared Capture Stream (48 kHz)               │
│               │                         │                   │
│               ▼                         ▼                   │
│     Local Apps (Discord)          Active Codec Selector     │
│                                      ├── Opus (5ms frame)   │
│                                      └── Raw PCM (48kHz 16b)│
│                                         │                   │
│                                         ▼                   │
│                              Binary Packetizer (0x4D53)     │
│                                         │                   │
│                                         ▼                   │
│                                UDP Socket Sender            │
+─────────────────────────────────────────┬───────────────────+
                                          │ Unicast UDP / LAN
                                          ▼
+─────────────────────────────────────────────────────────────+
│                         HOST DEVICE                         │
│                    (Windows 10/11 / macOS)                  │
│                                                             │
│                        UDP Socket Receiver                  │
│                                 │                           │
│                                 ▼                           │
│                     Adaptive Jitter Buffer                  │
│                                 │                           │
│                                 ▼                           │
│              Watermark Monitor & Drift Controller           │
│                                 │                           │
│                                 ▼                           │
│                 Dynamic Resampler (rubato Sinc)             │
│                                 │                           │
│                                 ▼                           │
│                        Active Codec Decoder                 │
│                         ├── Opus Decoder + PLC              │
│                         └── Raw PCM Direct Pass             │
│                                 │                           │
│                                 ▼                           │
│                      CPAL Playback Engine                   │
│                                 │                           │
│                                 ▼                           │
│                 Virtual Audio Device Input Sink             │
│                   ├── Windows: CABLE Input (VB-Audio)       │
│                   └── macOS: BlackHole 2ch                  │
│                                 │                           │
│                                 ▼                           │
│           Host Applications (Games / Discord / Sunshine)    │
+─────────────────────────────────────────────────────────────+
```

---

## 4. Audio Pipeline & Latency Budget

### 4.1 Latency Stages Explained
1. **Hardware Capture & OS Audio Buffer**:
   - `cpal` opens input streams with shared buffer sizes targeting 128 to 240 samples at 48,000 Hz (~2.7ms to 5.0ms).
2. **Codec Algorithmic Delay & Lookahead**:
   - **Opus Low-Delay**: Configured with `OPUS_APPLICATION_RESTRICTED_LOWDELAY` using 5.0ms frames (240 samples). CELT mode lookahead is ~2.5ms. Total algorithmic delay: `7.5ms`.
   - **Raw PCM**: Audio samples are framed directly into UDP packets without encoding or lookahead. Algorithmic delay: `0.0ms`.
3. **LAN UDP Network Transit**:
   - Unicast UDP transmission across Gigabit Ethernet or 5GHz Wi-Fi 6 typically measures between `0.5ms` and `1.5ms`.
4. **Adaptive Jitter Buffer**:
   - Target watermark is dynamically calibrated between `5.0ms` (240 samples) and `10.0ms` (480 samples) to absorb network jitter while keeping latency minimal.
5. **Dynamic Drift Resampling**:
   - Rubato sinc interpolation operates on small chunk sizes (64–128 samples), adding less than `0.5ms` algorithmic delay.
6. **Virtual Audio Sink Playback Buffer**:
   - Audio samples written to the virtual loopback device (VB-Cable / BlackHole) buffer at ~`5.0ms`.

### 4.2 Comprehensive Latency Budget Comparison

| Stage | Opus Low-Delay (`5.0ms` Frame) | Raw PCM Mode |
| :--- | :--- | :--- |
| **Input Capture Buffer** | ~`2.7ms – 5.0ms` | ~`2.7ms` (128 samples @ 48 kHz) |
| **Codec Framing & Lookahead** | ~`7.5ms` (`5ms` frame + `2.5ms` CELT) | `0.0ms` (No codec overhead) |
| **LAN Transit (UDP)** | ~`0.5ms – 1.5ms` | ~`0.5ms – 1.5ms` |
| **Adaptive Jitter Buffer** | ~`5.0ms` | ~`5.0ms` |
| **Dynamic Resampling** | ~`0.5ms` | ~`0.5ms` |
| **Host Output Playback Buffer** | ~`5.0ms` | ~`5.0ms` |
| **Total End-to-End Latency** | **~`21.2ms – 24.5ms`** | **~`13.7ms – 14.7ms`** |
| **Bandwidth (48 kHz Mono)** | **~`64 – 128 kbps`** | **~`768 kbps`** |
| **Packet Loss Resilience** | **High** (Native Opus PLC) | **Low** (Interpolated Silence / Repeat) |

---

## 5. Audio Capture & Virtual Playback Engine

### 5.1 Shared Input Capture (`src-tauri/src/audio/capture.rs`)
- Built using the cross-platform Rust audio library `cpal`.
- **Operating Modes**:
  - **Windows**: Uses WASAPI Shared Mode (`AUDCLNT_SHAREMODE_SHARED`). Does not acquire exclusive hardware locks, guaranteeing local applications (Discord, web browsers) retain simultaneous microphone access.
  - **macOS**: Uses CoreAudio HAL shared input streams.
- **Audio Stream Parameters**:
  - Sample Rate: `48,000 Hz` (standard broadcast and Opus native rate).
  - Channels: Captured in mono or stereo, normalized to mono for network transmission to conserve bandwidth.
  - Sample Format: Converted internally from hardware format (`f32` or `i16`) into normalized `f32` for VU metering and `i16` for transmission.

### 5.2 Virtual Audio Playback Engine (`src-tauri/src/audio/playback.rs`)
- Outputs decoded audio into a virtual loopback audio device on the host.
- **Windows Integration**:
  - MicStream plays audio into **`CABLE Input (VB-Audio Virtual Cable)`**.
  - Host applications (e.g., Discord, games, Moonlight/Sunshine audio input) select **`CABLE Output (VB-Audio Virtual Cable)`** as their microphone.
- **macOS Integration**:
  - MicStream plays audio into **`BlackHole 2ch`**.
  - Host applications select **`BlackHole 2ch`** as their input device.
- **Virtual Device Detection**:
  - `src-tauri/src/audio/devices.rs` inspects audio endpoints on startup.
  - If a virtual device is absent, the UI displays an intuitive setup guide with installation links.

---

## 6. Dual-Mode Audio Codec Architecture

### 6.1 Opus Low-Delay Codec (`src-tauri/src/codec/opus_codec.rs`)
- Uses `audiopus` (safe Rust bindings to `libopus`).
- **Configuration**:
  - Mode: `OPUS_APPLICATION_RESTRICTED_LOWDELAY` (enforces CELT mode, bypassing SILK speech lookahead).
  - Sample Rate: `48,000 Hz`.
  - Frame Duration: `5.0ms` (`240` samples per channel).
  - Bitrate: Variable Bitrate (`VBR`) constrained to `96 kbps`.
  - Complexity: `5` (balanced low CPU usage and high quality).
- **Packet Loss Concealment (PLC)**:
  - When the host detects a missing sequence number, the Opus decoder is invoked with a null pointer (`decode(None, ...)`), allowing Opus to synthesize the missing frame seamlessly based on previous audio energy.

### 6.2 Raw PCM Codec (`src-tauri/src/codec/pcm_codec.rs`)
- Bypasses compression completely.
- Audio samples are packaged as 16-bit signed integers (Little-Endian) in 128 to 240 sample frames.
- Provides absolute lowest possible latency (~14ms end-to-end) on wired gigabit LANs or stable 5GHz Wi-Fi.

---

## 7. Binary Network Transport Protocol

### 7.1 Binary Packet Framing (`src-tauri/src/protocol/packet.rs`)
Every UDP packet begins with an efficient 16-byte fixed-size binary header:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic (0x4D53)       |   Version (1) |  Payload Type |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Sequence Number                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Timestamp (Microseconds)                 |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         Payload Length        |           Reserved            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Audio Payload                         |
|                             ....                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

#### Field Specifications:
- **`Magic` (`2 bytes`)**: `0x4D53` (`'M'`, `'S'`) — identifies MicStream packets and discards extraneous LAN noise.
- **`Version` (`1 byte`)**: Protocol version `0x01`.
- **`Payload Type` (`1 byte`)**:
  - `0x00`: Opus low-delay frame.
  - `0x01`: Raw PCM frame (16-bit signed LE, 48 kHz mono).
  - `0x10`: Heartbeat PING.
  - `0x11`: Heartbeat PONG.
- **`Sequence Number` (`4 bytes`)**: Monotonically increasing `u32` for packet order verification and loss calculation.
- **`Timestamp` (`8 bytes`)**: `u64` microsecond timestamp from client audio clock for jitter buffer synchronization.
- **`Payload Length` (`2 bytes`)**: Length in bytes of the following audio payload (`u16`).
- **`Reserved` (`2 bytes`)**: Alignment and future protocol extensions (`0x0000`).

### 7.2 Heartbeat & Telemetry Protocol (`src-tauri/src/net/heartbeat.rs`)
- Periodic PING packets sent every 1,000ms.
- Receiver echoes PONG containing the client's transmit timestamp.
- Client computes Round-Trip Time (`RTT`) and moving packet loss rate, emitting telemetry events to the frontend.

---

## 8. Adaptive Jitter Buffering & Dynamic Drift Resampling

### 8.1 The Hardware Clock Drift Problem
Client and host audio hardware rely on distinct physical crystal oscillators. Even when both are configured for `48,000 Hz`, real-world hardware clocks drift:
- Client capture clock: `48,004 Hz`
- Host playback clock: `47,996 Hz`
- Over 1 hour of streaming, this 8 Hz difference results in `28,800` excess samples (~600ms of accumulated delay) or buffer depletion clicks.

```
Hardware Clock Mismatch (e.g. +8 Hz)
          │
          ▼
Buffer Watermark Trend Detection (5s Window)
          │
          ▼
Proportional Drift Controller: Compute Resampling Ratio (e.g. 1.00016)
          │
          ▼
Rubato Bandlimited Sinc Interpolator (Smooth Rate Adjustment)
          │
          ▼
Continuous Glitch-Free Audio Output with Zero Drift
```

### 8.2 Adaptive Jitter Buffer (`src-tauri/src/audio/jitter_buffer.rs`)
- Ring buffer with watermark target of `240` samples (`5.0ms`).
- Computes an exponential moving average (EMA) of buffer occupancy:
  $$\text{EMA}_t = \alpha \cdot \text{Occupancy}_t + (1 - \alpha) \cdot \text{EMA}_{t-1}$$
- If short-term network jitter increases packet variance, the target watermark smoothly expands up to `10.0ms` to prevent buffer underruns, then slowly settles back to `5.0ms`.

### 8.3 Dynamic Resampler (`src-tauri/src/audio/resampler.rs`)
- Integrates `rubato` (bandlimited sinc resampler).
- When the buffer watermark trends above the target depth, the resampler increases output sample production slightly (e.g., ratio `1.0001`); when below target depth, it decreases production slightly.
- Sinc interpolation preserves pristine audio quality without harmonic distortion or pitch bending.

---

## 9. Zero-Configuration LAN Discovery (mDNS) & Handshake

### 9.1 mDNS Service Advertisement (`src-tauri/src/net/discovery.rs`)
- When running in **Host mode**, MicStream advertises an mDNS service:
  - Service Type: `_micstream._udp.local.`
  - Service Name: `MicStream-<HostName>`
  - Port: Default `48124` (or user-defined in Advanced Settings).
  - TXT Records:
    ```
    version=1
    role=host
    codec=opus
    hostname=<Computer Name>
    ```

### 9.2 Client Discovery & Manual Fallback
- In **Client mode**, the client listens for `_micstream._udp` services across all active network interfaces.
- Discovered hosts are displayed in a reactive UI list with hostname, IP, and port.
- **Manual IP Fallback**: Users can enter a direct IP and port if multicast traffic is filtered by corporate or managed Wi-Fi routers. Previously connected hosts are persisted to local storage.

---

## 10. Desktop Application & UI/UX Design

### 10.1 UI Principles
- **Modern Minimalist Dark Theme**: Dark slate color palette, clean typography, and responsive layout.
- **Single-Window Simplicity**: The main screen contains only essential controls: Role toggle, device dropdown, discovery list, and live VU meter.
- **Advanced Settings Drawer**: Deep configuration (codec mode, custom ports, buffer depths, manual IP) is tucked into a dedicated modal.

### 10.2 Component Hierarchy (`src/`)

```
App.tsx
├── Header.tsx                 # Logo, status badge, role toggle, settings button
├── RoleSelector.tsx           # Switch between "Client (Sender)" and "Host (Receiver)"
├── Dashboard.tsx              # Main operational view depending on role:
│   ├── [Client View]:
│   │   ├── DevicePicker.tsx   # Microphone selection dropdown
│   │   ├── VolumeMeter.tsx    # Real-time input audio VU meter
│   │   ├── HostList.tsx       # Discovered mDNS hosts + manual entry
│   │   └── StreamControl.tsx  # Connect / Disconnect button
│   └── [Host View]:
│       ├── DevicePicker.tsx   # Output virtual cable selection dropdown
│       ├── HostSetupCard.tsx  # Missing driver detector & installation guide
│       ├── VolumeMeter.tsx    # Real-time output audio VU meter
│       └── ListenControl.tsx  # Start / Stop listening button
├── ConnectionStats.tsx        # Telemetry badge: RTT latency, packet loss, codec
└── AdvancedModal.tsx          # Codec toggle (Opus/PCM), buffer sizing, port input
```

### 10.3 IPC Command & Event Interface

| Command / Event | Direction | Description |
| :--- | :--- | :--- |
| `get_audio_devices` | Frontend $\to$ Backend | Returns lists of available input and output devices. |
| `start_stream` | Frontend $\to$ Backend | Starts capture and UDP transmission to host. |
| `stop_stream` | Frontend $\to$ Backend | Stops capture and socket transmission. |
| `start_listen` | Frontend $\to$ Backend | Starts UDP receiver and virtual playback sink. |
| `stop_listen` | Frontend $\to$ Backend | Stops UDP receiver and playback sink. |
| `audio-level` | Backend $\to$ Frontend | Emits RMS/Peak audio level for live VU meter (60 Hz). |
| `telemetry-update` | Backend $\to$ Frontend | Emits RTT latency, jitter, and packet loss stats (1 Hz). |
| `host-discovered` | Backend $\to$ Frontend | Emits newly detected mDNS host on local network. |

---

## 11. Project Directory & Module Structure

```
MicStream/
├── Cargo.toml                       # Cargo workspace definition
├── README.md                        # Quickstart, overview, and setup guide
├── docs/
│   └── masterplan.md                # This document
├── src-tauri/                       # Rust backend
│   ├── Cargo.toml                   # Tauri & Rust crate dependencies
│   ├── tauri.conf.json              # Tauri application configuration
│   ├── capabilities/                # Tauri v2 security capabilities
│   │   └── default.json
│   └── src/
│       ├── main.rs                  # Application bootstrap
│       ├── lib.rs                   # Library root & Tauri command handlers
│       ├── state.rs                 # Shared thread-safe application state
│       ├── audio/
│       │   ├── mod.rs               # Audio subsystem module root
│       │   ├── capture.rs           # CPAL shared microphone capture stream
│       │   ├── playback.rs          # CPAL virtual loopback playback stream
│       │   ├── devices.rs           # Device enumeration and virtual driver check
│       │   ├── jitter_buffer.rs     # Adaptive watermark jitter buffer
│       │   └── resampler.rs         # rubato sinc dynamic micro-resampler
│       ├── codec/
│       │   ├── mod.rs               # Codec subsystem module root
│       │   ├── opus_codec.rs        # Opus 5ms low-delay encoder & decoder + PLC
│       │   └── pcm_codec.rs         # Raw 48kHz 16-bit PCM framing
│       ├── net/
│       │   ├── mod.rs               # Network subsystem module root
│       │   ├── transport.rs         # UDP socket sender and receiver routines
│       │   ├── discovery.rs         # mDNS service announcement & browse loops
│       │   └── heartbeat.rs         # RTT ping/pong & packet loss tracker
│       └── protocol/
│           ├── mod.rs               # Protocol module root
│           └── packet.rs            # 16-byte binary header framing & parser
└── src/                             # Frontend UI (React + TypeScript + Tailwind)
    ├── index.html                   # HTML entry point
    ├── vite.config.ts               # Vite configuration
    ├── package.json                 # Node dependencies
    ├── tsconfig.json                # TypeScript compiler configuration
    ├── src/
    │   ├── main.tsx                 # React DOM mount point
    │   ├── App.tsx                  # App shell & router
    │   ├── components/              # Modular UI components
    │   ├── hooks/                   # React hooks for Tauri IPC bindings
    │   ├── types/                   # Shared TypeScript interfaces
    │   └── styles/                  # Tailwind CSS styling
```

---

## 12. Milestone Roadmap & Testable Deliverables

```
+─────────────────────────────────────────────────────────────+
│                       ROADMAP PHASES                        │
│                                                             │
│  [M1] Scaffolding & Prototypes                              │
│    │  (Cargo workspace, Tauri v2, documentation)             │
│    ▼                                                        │
│  [M2] Audio Engine                                          │
│    │  (Shared capture, virtual playback, driver detection)  │
│    ▼                                                        │
│  [M3] Transport & Clock Sync                                │
│    │  (UDP, Opus 5ms, Raw PCM, jitter buffer, rubato)       │
│    ▼                                                        │
│  [M4] Discovery & Connection                                │
│    │  (mDNS, manual IP fallback, heartbeat telemetry)       │
│    ▼                                                        │
│  [M5] UI & MVP Release                                      │
│       (VU meter, connection stats, settings modal, release) │
+─────────────────────────────────────────────────────────────+
```

### Milestone 1: Core Scaffolding, Documentation & Prototyping
- **Objective**: Establish project repository, build pipelines, and documentation.
- **Tasks**:
  1. Author `README.md` and `docs/masterplan.md`.
  2. Initialize Tauri v2 workspace with React, TypeScript, and Tailwind CSS.
  3. Configure `src-tauri/Cargo.toml` with dependencies (`cpal`, `audiopus`, `rubato`, `tokio`, `mdns-sd`).
  4. Build standalone audio loopback test verifying `cpal` functionality on host OS.
- **Testable Deliverables**:
  - `cargo check` and `npm run build` execute cleanly.
  - Test binary captures microphone samples and echoes them to headphones.

### Milestone 2: Audio Engine (Shared Capture & Virtual Playback)
- **Objective**: Implement non-exclusive input capture and virtual device output.
- **Tasks**:
  1. Implement device enumeration (`src-tauri/src/audio/devices.rs`).
  2. Implement shared microphone capture (`src-tauri/src/audio/capture.rs`).
  3. Implement virtual audio playback sink (`src-tauri/src/audio/playback.rs`).
  4. Implement loopback driver validator for VB-Cable and BlackHole.
- **Testable Deliverables**:
  - Unit tests for format conversions (`f32` to `i16`).
  - Integration test: Discord captures client mic while MicStream captures concurrently without errors.

### Milestone 3: Real-Time Networking & Dynamic Sync
- **Objective**: Sub-25ms audio streaming over UDP with clock synchronization.
- **Tasks**:
  1. Implement binary packet header (`0x4D53`) serialization/deserialization.
  2. Implement Opus low-delay codec (5ms frames) and Raw PCM framing.
  3. Build UDP socket transport loops using `tokio::net::UdpSocket`.
  4. Implement adaptive jitter buffer and `rubato` drift resampler.
- **Testable Deliverables**:
  - Unit test verifying Opus encoding and decoding of synthetic sine waves with cross-correlation > 0.98.
  - Simulation test: 1-hour continuous packet stream with synthetic clock drift (+10 Hz) maintains target buffer watermark without overflow or underflow.

### Milestone 4: Zero-Conf Discovery & Connection Management
- **Objective**: Frictionless LAN host discovery and connection monitoring.
- **Tasks**:
  1. Implement mDNS advertiser on Host broadcasting `_micstream._udp`.
  2. Implement mDNS browser on Client detecting LAN hosts in real time.
  3. Implement manual IP:port fallback and connection history persistence.
  4. Implement UDP heartbeat tracking RTT latency and packet loss.
- **Testable Deliverables**:
  - Launching Host on Windows automatically appears in Client discovery list on macOS within 2 seconds.
  - Heartbeat accurately reflects network RTT and detects simulated dropped packets.

### Milestone 5: Polished UI & MVP Delivery
- **Objective**: Complete plug-and-play desktop application.
- **Tasks**:
  1. Build unified Dashboard with role toggle and device selection.
  2. Implement real-time VU meter (60 Hz) and connection health telemetry.
  3. Build Host Setup Guide modal for virtual audio cable onboarding.
  4. Build Advanced Settings modal for codec toggle and custom ports.
  5. Package release bundles (`.dmg` for macOS Apple Silicon, `.msi`/`.exe` for Windows).
- **Testable Deliverables**:
  - Complete end-to-end user verification: Mac client streams voice to Windows host during Moonlight gaming session.

---

## 13. Testing, Benchmarking & Verification Strategy

### 13.1 Automated Unit Tests
- **Packet Serialization**: Verify packing, byte alignment, sequence wrapping, and timestamp decoding.
- **Codec Round-Trip**: Pass test audio through Opus encoder/decoder; verify waveform similarity.
- **Jitter Buffer Watermark Math**: Feed synthetic packet arrivals with jitter; verify watermark smoothing.

### 13.2 Acoustic Pulse Loopback Test (Latency Verification)

```
[Pulse Generator] ───► [Client Microphone]
                              │
                              ▼ (MicStream UDP Pipeline)
                              ▼
                       [Host Virtual Sink]
                              │
                              ▼
[Dual-Channel Audio Recorder / Audacity / Oscilloscope]
 Channel 1: Pulse trigger onset
 Channel 2: Virtual cable output signal
 Delta (t2 - t1) = Measured End-to-End Latency
 Target: < 25ms (Opus) / < 16ms (Raw PCM)
```

### 13.3 Manual Verification Scenarios
1. **Concurrent Local Discord**: Client runs active Discord voice call; MicStream streams same mic to Host. Both receive clear voice.
2. **Cross-Platform Matrix**: Verify Mac $\to$ Windows, Windows $\to$ Windows, Windows $\to$ Mac, Mac $\to$ Mac.
3. **Hot-Plugging**: Disconnect active headset microphone while streaming; reconnect and switch input cleanly without crash.
4. **Network Disruption**: Disconnect Wi-Fi for 5 seconds; verify reconnection without stuck streams or audio distortion.

---

## 14. MVP Acceptance Criteria

1. **Plug & Play Discovery**: Host started on Windows PC with VB-Cable installed is discovered by Mac client on local Wi-Fi within 2 seconds.
2. **Concurrent Mic Access**: Client can use the microphone in local Discord while streaming to host with zero device contention.
3. **Seamless Host Voice**: Games and voice chat on the host receive voice from the virtual audio device without audible lag or sync issues with Moonlight video.
4. **Latency Verification**: Total measured latency under `25ms` with Opus low-delay and under `16ms` with Raw PCM.
5. **Continuous Streaming Stability**: Stream runs for 2+ hours without drift buffer bloat, underrun clicks, or crashes.
