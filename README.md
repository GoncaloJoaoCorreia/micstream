# MicStream

> **Ultra-low latency, cross-platform LAN microphone streaming utility designed for Moonlight & Apollo game streaming.**

MicStream captures microphone audio from a client computer (such as a MacBook or laptop) and streams it with minimal latency over a local network to a host machine (such as a Windows gaming PC), injecting it directly into a virtual audio input device (VB-Audio Cable on Windows or BlackHole on macOS). Host-side games, Discord, and voice communication tools treat this stream as a physical microphone input.

---

## Highlights

- **Ultra-Low Latency Pipeline**: Engineered for sub-`25ms` end-to-end latency with low-delay Opus (5ms frames) and sub-`16ms` with uncompressed Raw PCM over LAN.
- **Dual Transport Modes**:
  - **Opus over UDP (Default)**: Bandwidth-efficient (~64–128 kbps), low algorithmic delay (5ms frame + 2.5ms lookahead), with built-in Packet Loss Concealment (PLC).
  - **Raw PCM over UDP (Advanced)**: Uncompressed 48 kHz 16-bit audio bypassing codecs completely for competitive, low-jitter LAN setups.
- **Non-Exclusive Microphone Capture**: Uses OS-level shared capture (WASAPI Shared on Windows, CoreAudio on macOS), allowing applications on the client machine (e.g., client-side Discord) to access the microphone concurrently without device locking.
- **Multi-Device Support**: Automatically enumerates all connected physical microphones (built-in, USB, 3.5mm jack, Bluetooth headsets) with hot-plug detection.
- **Zero-Configuration Discovery (mDNS)**: Automatically announces and discovers MicStream hosts on the local network (`_micstream._udp`), with manual IP:port fallback and persistent connection history.
- **Dynamic Clock Drift Compensation**: Absorbs crystal oscillator discrepancies between client and host audio hardware using bandlimited sinc interpolation (`rubato`), preventing buffer bloat, dropouts, or audible clicks during long gaming sessions.
- **Single Unified Application**: One lightweight binary toggles seamlessly between **Client (Sender)** and **Host (Receiver)** modes.
- **Modern User Experience**: Built on Tauri v2 and React with Tailwind CSS, featuring live VU audio level meters, real-time RTT latency and packet loss metrics, and an onboarding guide for virtual audio drivers.

---

## Architecture Overview

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

### Latency Budget Breakdown

| Processing Stage | Opus Low-Delay (`5ms` frame) | Raw PCM Mode |
| :--- | :--- | :--- |
| **Capture Buffer (CPAL Shared)** | ~`2.7ms – 5.0ms` | ~`2.7ms` (128 samples @ 48 kHz) |
| **Codec Framing & Lookahead** | ~`7.5ms` (5.0ms frame + 2.5ms CELT) | `0.0ms` (No codec delay) |
| **LAN Network Transit (UDP)** | ~`0.5ms – 1.5ms` | ~`0.5ms – 1.5ms` |
| **Adaptive Jitter Buffer** | ~`5.0ms` | ~`5.0ms` |
| **Playback to Virtual Sink** | ~`5.0ms` | ~`5.0ms` |
| **Total Estimated Latency** | **~`20.7ms – 24.0ms`** | **~`13.2ms – 14.2ms`** |

---

## Virtual Audio Device Setup (Host Only)

To pipe audio into host applications, a virtual audio loopback driver is required on the machine running in **Host mode**:

### Windows 10/11
1. Download and install **[VB-Audio Virtual Cable](https://vb-audio.com/Cable/)** (free).
2. In **MicStream (Host Mode)**, set the audio output device to `CABLE Input (VB-Audio Virtual Cable)`.
3. In your host game, Discord, or Windows sound settings, select `CABLE Output (VB-Audio Virtual Cable)` as your recording microphone.

### macOS (Apple Silicon)
1. Install **[BlackHole 2ch](https://github.com/ExistentialAudio/BlackHole)** via Homebrew:
   ```bash
   brew install blackhole-2ch
   ```
   *(Or download the installer PKG directly from Existential Audio).*
2. In **MicStream (Host Mode)**, set the output device to `BlackHole 2ch`.
3. In your host voice software, select `BlackHole 2ch` as the input microphone.

> *Note: MicStream automatically scans for these virtual devices on startup and provides visual status cues if they are not yet installed.*

---

## Tech Stack

| Layer | Technologies | Rationale |
| :--- | :--- | :--- |
| **Backend & Audio Core** | **Rust** (`cpal`, `audiopus`, `rubato`, `tokio`, `mdns-sd`) | Zero garbage-collection pauses, memory safety, deterministic real-time audio threads, and low CPU usage (<2%). |
| **Desktop Wrapper** | **Tauri v2** | Native webview rendering, small memory footprint (<90 MB), hardened IPC security, and cross-platform native bundle generation. |
| **Frontend UI** | **React**, **TypeScript**, **Tailwind CSS**, **Lucide Icons** | Responsive, modern dark UI with real-time VU visualizers and low-overhead state management. |

---

## Repository Structure

```
MicStream/
├── .github/                     # GitHub Actions workflows and release scripts
│   ├── scripts/
│   │   └── extract-changelog.sh # Release notes extractor and validator
│   └── workflows/
│       └── release.yml          # Multi-platform release CI pipeline
├── docs/
│   └── masterplan.md            # Comprehensive architecture, protocol, and roadmap
├── CHANGELOG.md                 # Keep a Changelog version history
├── src-tauri/                   # Rust backend
│   ├── Cargo.toml               # Cargo manifest & dependencies
│   ├── tauri.conf.json          # Tauri configuration (window, permissions, bundle)
│   └── src/
│       ├── main.rs              # Application entry point
│       ├── lib.rs               # Library root and Tauri command bindings
│       ├── state.rs             # Application state management
│       ├── audio/
│       │   ├── capture.rs       # CPAL shared microphone capture
│       │   ├── playback.rs      # CPAL virtual sink playback
│       │   ├── devices.rs       # Audio device enumeration & hot-plug detection
│       │   ├── jitter_buffer.rs # Adaptive jitter buffer with watermark tracking
│       │   └── resampler.rs     # rubato dynamic sinc micro-resampling
│       ├── codec/
│       │   ├── opus_codec.rs    # Low-delay Opus encoder & decoder wrapper
│       │   └── pcm_codec.rs     # Raw PCM framing & uncompressed pass-through
│       ├── net/
│       │   ├── transport.rs     # Asynchronous UDP socket sender & receiver
│       │   ├── discovery.rs     # mDNS service announcement & browsing
│       │   └── heartbeat.rs     # RTT latency & packet loss telemetry
│       └── protocol/
│           └── packet.rs        # Binary packet framing (0x4D53 header)
├── src/                         # Frontend UI (React + Tailwind CSS)
│   ├── App.tsx                  # Main router & role toggle container
│   ├── components/
│   │   ├── Header.tsx           # Title bar & role status indicator
│   │   ├── RoleSelector.tsx     # Client / Host mode toggle
│   │   ├── DevicePicker.tsx     # Audio device dropdown selector
│   │   ├── VolumeMeter.tsx      # Real-time VU meter
│   │   ├── ConnectionStats.tsx  # Latency, packet loss, codec display
│   │   ├── HostSetupCard.tsx    # VB-Cable / BlackHole helper guide
│   │   └── AdvancedModal.tsx    # Codec, port, buffer, and manual IP settings
│   ├── hooks/                   # Custom React hooks (devices, discovery, streaming)
│   └── styles/                  # Tailwind CSS styling
└── README.md                    # Project overview & documentation
```

---

## Development & Building

### Prerequisites

1. **Rust**: Ensure Rust 1.75+ is installed (`rustup update stable`).
2. **Node.js**: Node.js 18+ and `npm` or `pnpm`.
3. **C Compiler**:
   - **macOS**: Xcode Command Line Tools (`xcode-select --install`).
   - **Windows**: Visual Studio 2022 with C++ Desktop Development tools.

### Running in Development

```bash
# 1. Install frontend dependencies
npm install

# 2. Launch the Tauri development environment (spawns Vite dev server + Rust backend)
npm run tauri dev
```

### Production Build

```bash
# Build the production desktop binary and installers (.dmg on macOS, .msi / .exe on Windows)
npm run tauri build
```

---

## Usage Guide

### Client (Sender) Mode
1. Launch **MicStream** and select **Client (Sender)**.
2. Select your microphone from the **Input Device** dropdown.
3. Test your voice level on the visual **VU Meter**.
4. In the **Available Hosts** list, click on your discovered host PC, or enter an IP address manually.
5. Click **Start Streaming**.

### Host (Receiver) Mode
1. Launch **MicStream** and select **Host (Receiver)**.
2. Select your virtual audio device (`CABLE Input` on Windows or `BlackHole 2ch` on macOS) as the **Output Device**.
3. Click **Start Listening**.
4. Open your game or voice app (e.g. Discord) and set the microphone input to `CABLE Output` (Windows) or `BlackHole 2ch` (macOS).

### Advanced Settings
Click the **Gear** icon to open the Advanced Settings dialog:
- **Audio Transport**: Switch between `Opus Low-Delay` (default) and `Raw PCM` (lowest latency LAN mode).
- **Target Jitter Buffer**: Adjust buffer depth (default: `5.0ms`, range: `2.5ms – 20.0ms`).
- **UDP Port**: Change default listening/transmission port (default: `48124`).
- **Manual IP Fallback**: Connect directly to hosts when mDNS multicast is blocked by network infrastructure.

---

## Release Process & Versioning

MicStream follows [Semantic Versioning 2.0.0](https://semver.org/) (`vMAJOR.MINOR.PATCH`) and adheres to the [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) standard.

### Automated CI/CD Release Pipeline
Native release binaries are compiled, tested, and published automatically via GitHub Actions on every push to the `main` branch.

```
+─────────────────────────────────────────────────────────────+
│                       Push to main                          │
+──────────────────────────────┬──────────────────────────────+
                               │
                               ▼
+─────────────────────────────────────────────────────────────+
│           prepare-release (ubuntu-latest, ~5 sec)           │
│   • Calculate next patch version (e.g. v0.1.0 -> v0.1.1)    │
│   • Validate CHANGELOG.md contains matching ## [X.Y.Z]      │
│   • Extract release notes to artifact                       │
+───────────────────────┬─────────────────────────────┬───────+
                        │                             │
                        ▼                             ▼
+──────────────────────────────+ +────────────────────────────+
│  macos-latest (matrix)       │ │  windows-latest (matrix)   │
│  • npm ci & cargo test       │ │  • npm ci & cargo test     │
│  • npm run tauri build       │ │  • npm run tauri build     │
│  • Upload .dmg artifact      │ │  • Upload NSIS .exe setup  │
+───────────────────────┬──────+ +─────────────────────┬──────+
                        │                             │
                        └──────────────┬──────────────┘
                                       │ All builds succeed
                                       ▼
+─────────────────────────────────────────────────────────────+
│           publish-release (ubuntu-latest)                   │
│   • Download macOS and Windows artifacts                    │
│   • Create annotated Git tag vX.Y.Z                         │
│   • Publish GitHub Release with notes and native installers │
+─────────────────────────────────────────────────────────────+
```

1. **Pre-flight Validation (`prepare-release`)**:
   - Computes the target version by incrementing the patch number from the latest git tag (or defaults to the version in `package.json` if no tags exist).
   - Validates that `CHANGELOG.md` contains an entry matching the target version (`## [X.Y.Z] - YYYY-MM-DD`). If the version heading is absent or empty, the workflow aborts immediately to conserve runner minutes.
   - Extracts the release notes into an artifact.

2. **Cross-Platform Matrix Build (`build-binaries`)**:
   - Compiles and tests the Rust workspace (`cargo test --workspace`) and builds the React/Vite frontend.
   - Generates native installers: `.dmg` for macOS and NSIS `.exe` (`*-setup.exe`) for Windows.
   - Uploads compiled binaries as workflow artifacts.

3. **Atomic Publication (`publish-release`)**:
   - Downloads all platform artifacts once all matrix jobs succeed. If any platform fails, no release or tag is created.
   - Creates the annotated Git tag `vX.Y.Z` and publishes the GitHub Release with attached installers and release notes.

### How to Stage and Release a New Version
1. Open `CHANGELOG.md`.
2. Move unreleased changes from `## [Unreleased]` into a new section matching the next patch version:
   ```markdown
   ## [Unreleased]

   ## [0.1.1] - 2026-09-22

   ### Added
   - Description of newly added feature.

   ### Fixed
   - Description of resolved bug fix.
   ```
3. Commit the updated `CHANGELOG.md` and push to `main` (or merge a PR into `main`):
   ```bash
   git add CHANGELOG.md
   git commit -m "docs: prepare changelog for v0.1.1"
   git push origin main
   ```
4. The GitHub Actions release workflow will trigger automatically, create tag `v0.1.1`, and publish the release with macOS and Windows binaries attached.

> **Note for macOS Users**: Unsigned `.dmg` releases may trigger Gatekeeper on first launch. Right-click (or Control-click) the application in Finder and choose **Open**, then click **Open** to confirm.

---

## Detailed Specifications

For in-depth details on the binary network protocol, dynamic sinc drift algorithms, testing protocols, and implementation milestones, refer to the master plan:

👉 **[docs/masterplan.md](docs/masterplan.md)**

---

## License

This project is licensed under the MIT License. See `LICENSE` for details.
