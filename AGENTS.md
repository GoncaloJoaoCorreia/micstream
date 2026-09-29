# AGENTS.md — Project Guide & Operational Context

This document provides system context, architectural invariants, module mappings, and workflow commands for AI agents and human contributors working on **MicStream**.

---

## 1. Project Overview & Objective

**MicStream** is an ultra-low latency (<25ms), cross-platform LAN microphone streaming utility engineered specifically for game streaming setups (e.g. **Moonlight**, **Apollo**, and **Sunshine**).

- **Problem Solved**: Upstream microphone streaming from client laptops (e.g. macOS Apple Silicon or low-power PCs) to remote Windows host gaming rigs without hardware cables or exclusive device locking.
- **Architecture**: One-way real-time audio pipeline over UDP, capturing shared microphone input on the client, transmitting Opus/PCM frames, and playing into virtual audio loopback drivers (`VB-Cable` / `BlackHole`) on the host.

---

## 2. Tech Stack & Architectural Decisions

| Layer | Technologies | Rationale & Trade-offs |
| :--- | :--- | :--- |
| **Backend Audio Core** | **Rust 2021** (`cpal`, `opus`, `rubato`, `tokio`, `mdns-sd`) | Zero garbage collection jitter, deterministic audio callback execution, thread-safe memory guarantees, and <2% CPU overhead. |
| **Desktop Shell** | **Tauri v2** | Native OS webview integration, lightweight memory usage (<90 MB), strict capability-based IPC security boundaries. |
| **Frontend UI** | **React 18**, **TypeScript**, **Tailwind CSS**, **Lucide Icons** | Component-driven UI with real-time 60 Hz VU audio meters, responsive state management, and clear role toggling. |
| **Virtual Audio Sink** | **VB-Audio Cable** (Win) / **BlackHole** (Mac) | Pre-existing certified virtual loopback drivers avoid dangerous custom kernel extensions or unsigned driver installation. |
| **License** | **The Unlicense** | Unencumbered public domain dedication across all code and assets. |

---

## 3. Repository & Module Structure

```
MicStream/
├── .github/                     # GitHub Actions CI/CD workflows and automation scripts
│   ├── scripts/
│   │   └── extract-changelog.sh # Validates and extracts version notes from CHANGELOG.md
│   └── workflows/
│       └── release.yml          # Multi-platform release CI pipeline (macOS .dmg + Windows .exe)
├── docs/
│   ├── ARCHITECTURE.md          # In-depth technical specification, packet framing & math
│   ├── INSTALL.md               # User-friendly installation & virtual audio setup guide
│   └── RELEASE.md               # CI/CD release workflow, matrix build flow & version staging
├── CHANGELOG.md                 # Version history adhering to Keep a Changelog standard
├── LICENSE                      # The Unlicense public domain dedication
├── package.json                 # Frontend dependencies and Tauri scripts
├── tsconfig.json                # TypeScript compiler configuration
├── vite.config.ts               # Vite configuration for Tauri frontend
├── tailwind.config.js           # Tailwind CSS styling configuration
├── src-tauri/                   # Rust backend workspace member
│   ├── Cargo.toml               # Cargo manifest & crate dependencies
│   ├── tauri.conf.json          # Tauri application, window, and bundle settings
│   ├── capabilities/            # Tauri v2 security capability definitions
│   │   └── default.json
│   └── src/
│       ├── main.rs              # Tauri application bootstrap
│       ├── lib.rs               # Library root and Tauri IPC command handlers
│       ├── state.rs             # Thread-safe global application state
│       ├── audio/
│       │   ├── mod.rs           # Audio subsystem module root
│       │   ├── capture.rs       # CPAL shared microphone capture (WASAPI / CoreAudio)
│       │   ├── playback.rs      # CPAL virtual sink playback engine
│       │   ├── devices.rs       # Audio device enumeration & hot-plug detection
│       │   ├── jitter_buffer.rs # Adaptive jitter buffer with watermark monitoring
│       │   └── resampler.rs     # rubato dynamic sinc micro-resampling for clock drift
│       ├── codec/
│       │   ├── mod.rs           # Codec subsystem module root
│       │   ├── opus_codec.rs    # Low-delay Opus encoder & decoder (5ms frames) + PLC
│       │   └── pcm_codec.rs     # Raw PCM framing & uncompressed pass-through
│       ├── net/
│       │   ├── mod.rs           # Network subsystem module root
│       │   ├── transport.rs     # Asynchronous UDP socket sender & receiver routines
│       │   ├── discovery.rs     # mDNS service announcement & browse loops
│       │   └── heartbeat.rs     # RTT latency & packet loss telemetry ping/pong
│       └── protocol/
│           ├── mod.rs           # Protocol subsystem module root
│           └── packet.rs        # Binary packet framing (0x4D53 header serialization)
└── src/                         # Frontend React UI
    ├── main.tsx                 # React DOM mount point
    ├── App.tsx                  # Root application router and role toggle
    ├── components/
    │   ├── Header.tsx           # Title bar & role status indicator
    │   ├── RoleSelector.tsx     # Client (Sender) / Host (Receiver) mode toggle
    │   ├── DevicePicker.tsx     # Audio device dropdown selector
    │   ├── VolumeMeter.tsx      # Real-time VU level visualizer (RMS / Peak)
    │   ├── ConnectionStats.tsx  # Latency, jitter, packet loss, and codec display
    │   ├── HostSetupCard.tsx    # VB-Cable / BlackHole helper guide card
    │   └── AdvancedModal.tsx    # Transport codec, port, buffer, and manual IP settings
    ├── hooks/                   # React custom hooks for Tauri IPC events & commands
    ├── types/                   # Shared TypeScript interface definitions
    └── styles/                  # Tailwind CSS global styles
```

---

## 4. Key Architectural Invariants & Rules

When modifying or extending the codebase, strictly adhere to these invariants:

1. **Single Unified Binary**:
   - The application must remain a single executable. Both Client (Sender) and Host (Receiver) modes must live in the same binary, toggled cleanly via UI state and Tauri commands.
2. **Non-Exclusive Audio Capture**:
   - Audio capture streams must ALWAYS use shared mode (`WASAPI Shared` on Windows, `CoreAudio` on macOS). Exclusive locks must never be acquired, allowing local apps (e.g. client Discord or Zoom) to use the microphone simultaneously.
3. **Real-Time Audio Thread Safety**:
   - Audio callbacks registered with `cpal` run on real-time OS audio threads.
   - **Never** perform heap allocations, file I/O, network I/O, or acquire blocking mutex locks inside the real-time audio callback. Use lock-free ringbuffers (`ringbuf`) to exchange samples between audio threads and asynchronous network tasks.
4. **Audio Format Standard**:
   - Standard pipeline sample rate is **48,000 Hz**, mono or stereo, 16-bit signed integer or 32-bit floating point. Default frame size for Opus is 5ms (240 samples at 48 kHz).
5. **Binary Transport Framing**:
   - All network packets begin with the 16-byte magic header `0x4D53` (`MS`), including version, payload type (Opus / PCM / Heartbeat), sequence number, timestamp, and payload length. Refer to `docs/ARCHITECTURE.md` for exact field offsets.
6. **Clock Drift Compensation**:
   - Hardware crystal oscillators between two physical computers drift by ~10–50 PPM. The host jitter buffer continuously tracks the sample watermark and adjusts `rubato` sinc resampler ratios dynamically by ±0.05% to prevent buffer underrun/overflow without audible pitch distortion.
7. **Unencrypted LAN Transport**:
   - Designed strictly for private LANs. Avoid adding TLS/encryption layers to the audio transport loop that would increase end-to-end latency.

---

## 5. Development Workflow & Commands

### Prerequisites
- Rust 1.75+ (`rustup update stable`)
- Node.js 18+ & `npm`
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Windows: Visual Studio 2022 C++ Build Tools

### Common Commands

```bash
# Install frontend dependencies
npm install

# Run application in development mode (launches Vite dev server + Tauri native window)
npm run tauri dev

# Run Rust unit and integration tests across all workspace crates
cargo test --workspace

# Run TypeScript type check and frontend build
npm run build

# Build production desktop installers (.dmg on macOS, NSIS setup .exe on Windows)
npm run tauri build

# Run standalone audio prototype CLI (if debugging cpal capture/playback directly)
cargo run --bin audio_prototype --manifest-path src-tauri/Cargo.toml

# Validate and extract changelog release notes for a target version
.github/scripts/extract-changelog.sh 0.1.1 CHANGELOG.md release-notes.md
```

---

## 6. Coding Style & Conventions

- **Rust**:
  - Follow standard Rust naming conventions (`snake_case` for functions/variables, `PascalCase` for structs/enums).
  - Use `thiserror` for recoverable subsystem errors and `anyhow` for top-level Tauri IPC command boundaries.
  - Instrument asynchronous routines with `tracing` macros (`trace!`, `debug!`, `info!`, `warn!`, `error!`).
- **TypeScript / React**:
  - Use TypeScript strict mode with explicit interfaces for all IPC payloads.
  - Follow functional component style with React hooks.
  - Use Tailwind CSS utility classes; avoid inline styles.
- **Git Commits & Releases**:
  - Follow Conventional Commits format (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`).
  - Follow the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) format in `CHANGELOG.md`.
  - **Automated CI/CD Release Invariants & Rules**:
    - Pushes and merges to `main` automatically trigger `.github/workflows/release.yml`.
    - The CI pipeline calculates the next release version (incrementing patch version from the latest `v*.*.*` tag) and validates `CHANGELOG.md` using `.github/scripts/extract-changelog.sh`.
    - **MANDATORY**: Any change pushed to `main` intended for release MUST contain an explicit `## [X.Y.Z] - YYYY-MM-DD` section in `CHANGELOG.md` matching the next version. If missing or empty, CI fails immediately at the `prepare-release` gate.
    - Before pushing to `main`, always verify changelog extraction locally:
      ```bash
      .github/scripts/extract-changelog.sh <TARGET_VERSION> CHANGELOG.md /dev/null
      ```
    - Keep version fields synchronized across `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
  - For full release procedures and matrix build details, see `docs/RELEASE.md`.

---

## 7. Documentation Index

| Document | Audience & Purpose |
| :--- | :--- |
| 📦 **[docs/INSTALL.md](docs/INSTALL.md)** | Step-by-step installation instructions, virtual audio setup (VB-Cable/BlackHole), and troubleshooting for end users. |
| 🏗️ **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** | In-depth technical specification, audio pipelines, latency budget math, binary packet framing (`0x4D53`), and sinc resamplers. |
| 🚀 **[docs/RELEASE.md](docs/RELEASE.md)** | Automated CI/CD release workflow, cross-platform matrix build pipeline, and version staging guide. |
| 🤖 **[AGENTS.md](AGENTS.md)** | Project guidelines, architectural invariants, full repository module map, and development commands for contributors and AI agents. |
| 📝 **[CHANGELOG.md](CHANGELOG.md)** | Version history adhering to the Keep a Changelog standard. |
| ⚖️ **[LICENSE](LICENSE)** | Public domain dedication under the Unlicense. |
