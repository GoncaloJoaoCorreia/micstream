# MicStream

> **Send your laptop's microphone to your gaming PC over your home network with near-zero delay.**

MicStream is an easy-to-use utility designed for game streaming setups (like **Moonlight**, **Apollo**, and **Sunshine**). It captures audio from your laptop or client computer's microphone and sends it over your local network directly to your gaming PC. 

Your PC games, Discord, and voice chat apps receive your voice just like a regular plug-in microphone — no long audio cables or complicated hardware needed.

> [!TIP]
> 📦 **Getting Started**: Check out our step-by-step **[Installation & Setup Guide](docs/INSTALL.md)** for detailed instructions on installing MicStream and setting up virtual audio on Windows and macOS.

---

## Highlights

- ⚡ **Near-Instant Voice Streaming**: Stream with ultra-low latency (<25ms) so your voice is always in sync with your gameplay and chat.
- 🎮 **Built for Game Streaming**: Designed specifically to provide the missing microphone link for Moonlight, Sunshine, and Apollo setups.
- 🔍 **Automatic Host Discovery**: Automatically finds and connects to your gaming PC on your home Wi-Fi or wired network — no IP addresses or complex network configuration required.
- 🎙️ **Works With Any Microphone**: Supports all physical microphones, including built-in laptop mics, USB desktop mics, 3.5mm headsets, and Bluetooth earbuds.
- 🔄 **Share Your Microphone**: Captures your voice without locking the mic, so apps on your laptop (like local Discord or Zoom) can still use it at the same time.
- 🔊 **Crystal-Clear & Reliable Audio**: Dynamic drift correction and packet loss handling prevent audio crackling, robotic voices, or dropouts during long gaming sessions.
- 🎛️ **All-in-One Simple App**: A single, lightweight app that easily toggles between **Client (Sender)** and **Host (Receiver)**, complete with live volume meters and connection stats.

---

## How It Works

```
[ Your Microphone ] 
        │
        ▼
[ Client Laptop (MicStream Sender) ]
        │
        │  (Home Network / Wi-Fi or Ethernet)
        ▼
[ Host Gaming PC (MicStream Receiver) ]
        │
        ▼
[ Virtual Audio Device (VB-Cable / BlackHole) ]
        │
        ▼
[ Host Game / Discord / Moonlight ]
```

1. **Client Device (Sender)**: Captures audio from your physical microphone and sends low-delay Opus or Raw PCM packets over your local network via UDP.
2. **Host Device (Receiver)**: Receives audio packets, absorbs jitter, corrects clock drift, and plays the audio directly into a virtual audio cable (`VB-Cable` on Windows or `BlackHole` on macOS).
3. **Host Applications**: Voice chat apps and games listen to the virtual audio cable as if it were a physical microphone plugged directly into your host PC.

---

## Quickstart & Usage

> 📖 *For complete step-by-step instructions and virtual driver configuration, refer to the **[Installation & Setup Guide](docs/INSTALL.md)**.*

### 1. Host (Receiver) Mode
1. Open **MicStream** on your gaming PC and select **Host (Receiver)**.
2. Select your virtual audio device (`CABLE Input` on Windows or `BlackHole 2ch` on macOS) as the **Output Device**.
3. Click **Start Listening**.
4. In Discord or your games, select `CABLE Output` (Windows) or `BlackHole 2ch` (macOS) as your microphone input.

### 2. Client (Sender) Mode
1. Open **MicStream** on your laptop and select **Client (Sender)**.
2. Select your physical microphone as the **Input Device** and test the live **VU Meter**.
3. Click on your host PC in the **Available Hosts** list (or enter its local IP address manually).
4. Click **Start Streaming**.

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

# 2. Launch the Tauri development environment (Vite dev server + Rust backend)
npm run tauri dev
```

### Building for Production

```bash
# Compile the production desktop binary and installers (.dmg on macOS, .exe setup on Windows)
npm run tauri build
```

---

## License

This project is dedicated to the public domain under the [Unlicense](https://unlicense.org/). See `LICENSE` for details.
