# Installation & Setup Guide

Welcome to the **MicStream** installation and setup guide. This document walks you through installing MicStream on your computers, setting up the required virtual audio drivers, and streaming your microphone audio across your local network.

---

## Table of Contents

1. [Understanding How MicStream Works](#understanding-how-micstream-works)
2. [Prerequisites](#prerequisites)
3. [Step 1: Install MicStream](#step-1-install-micstream)
   - [macOS Installation](#macos-installation)
   - [Windows Installation](#windows-installation)
4. [Step 2: Virtual Audio Setup (Host PC Only)](#step-2-virtual-audio-setup-host-pc-only)
   - [Windows Setup (VB-Audio Cable)](#windows-setup-vb-audio-cable)
   - [macOS Setup (BlackHole)](#macos-setup-blackhole)
5. [Step 3: Connect and Stream Audio](#step-3-connect-and-stream-audio)
6. [Step 4: Configure Games & Discord](#step-4-configure-games--discord)
7. [Troubleshooting & FAQ](#troubleshooting--faq)

---

## Understanding How MicStream Works

MicStream connects two computers over your home network:

- **Client (Sender)**: The computer you are physically speaking into (e.g., your MacBook or laptop).
- **Host (Receiver)**: The remote computer running your game, Discord, or streaming software (e.g., your Windows gaming PC).

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

---

## Prerequisites

- **Network**: Both computers must be connected to the same local network (Wi-Fi or wired Ethernet).
- **Operating Systems**:
  - **macOS**: macOS 12 (Monterey) or newer (Apple Silicon & Intel supported).
  - **Windows**: Windows 10 or Windows 11 (64-bit).

---

## Step 1: Install MicStream

Download the latest release for your platform from the [GitHub Releases page](https://github.com).

### macOS Installation

1. Download the `.dmg` installer file (e.g., `MicStream_x.y.z_aarch64.dmg` or `MicStream_x.y.z_x64.dmg`).
2. Open the downloaded `.dmg` file and drag **MicStream** into your **Applications** folder.
3. Launch **MicStream** from Applications or Spotlight.
4. **First-launch Permission & Security**:
   - When prompted, grant MicStream permission to access your **Microphone**.
   - *Note for unsigned builds*: Because MicStream uses ad-hoc signing, macOS Gatekeeper may show a warning when opening the downloaded app for the first time. If blocked:
     - Open **System Settings** > **Privacy & Security**, scroll down to the **Security** section, and click **Open Anyway**.
     - Or remove the quarantine attribute via Terminal:
       ```bash
       xattr -cr /Applications/MicStream.app
       ```

### Windows Installation

1. Download the Windows setup installer (e.g., `MicStream_x.y.z_x64-setup.exe`).
2. Double-click the installer and follow the on-screen setup prompts.
3. **Windows SmartScreen Prompt**:
   - If Windows Defender SmartScreen appears with "Windows protected your PC", click **More info** and then click **Run anyway**.
4. Once installation completes, launch **MicStream** from the Start Menu or desktop shortcut.

---

## Step 2: Virtual Audio Setup (Host PC Only)

> 💡 **Why is this needed?**  
> To let games, Discord, or Windows/macOS listen to audio arriving from MicStream, you need a free "virtual audio cable". Think of it like a virtual patch cord: MicStream plugs into one end, and your games listen to the other end as if it were a physical microphone.

This step is **only required on the computer running as the Host (Receiver)**.

### Windows Setup (VB-Audio Cable)

1. **Download VB-Audio Virtual Cable**:
   - Visit the official [VB-Audio Cable Download Page](https://vb-audio.com/Cable/).
   - Download the free **VBCABLE_Driver_Pack.zip**.
2. **Install the Driver**:
   - Extract the downloaded `.zip` file into a folder.
   - Right-click `VBCABLE_Setup_x64.exe` and select **Run as administrator**.
   - Click **Install Driver** and confirm any security prompts.
   - *Recommendation*: Restart your PC after installation completes.
3. **Configure MicStream Host**:
   - Open MicStream and select **Host (Receiver)** mode.
   - Set the **Output Device** to `CABLE Input (VB-Audio Virtual Cable)`.
4. **Set Up Your Apps**:
   - In Discord, your games, or Windows Sound Settings, select `CABLE Output (VB-Audio Virtual Cable)` as your **Input Device / Microphone**.

### macOS Setup (BlackHole)

1. **Install BlackHole 2ch**:
   - **Using Homebrew** (easiest):
     ```bash
     brew install blackhole-2ch
     ```
   - **Or using the Installer PKG**:
     - Download the BlackHole installer directly from [Existential Audio](https://github.com/ExistentialAudio/BlackHole).
     - Run the `.pkg` installer and follow the installation wizard.
2. **Configure MicStream Host**:
   - Open MicStream and select **Host (Receiver)** mode.
   - Set the **Output Device** to `BlackHole 2ch`.
3. **Set Up Your Apps**:
   - In your voice chat software (e.g., Discord or game streaming apps), select `BlackHole 2ch` as your **Input Device / Microphone**.

---

## Step 3: Connect and Stream Audio

Now that both computers have MicStream installed and the Host has a virtual audio device ready:

### 1. Start the Host (Receiver)
1. On your gaming PC / host computer, launch **MicStream**.
2. Click **Host (Receiver)** at the top.
3. Select your virtual device (`CABLE Input` on Windows or `BlackHole 2ch` on macOS) in the **Output Device** dropdown.
4. Click **Start Listening**.

### 2. Start the Client (Sender)
1. On your laptop / client computer, launch **MicStream**.
2. Click **Client (Sender)** at the top.
3. Select your physical microphone from the **Input Device** dropdown.
4. Speak into your microphone and verify that the green **VU Meter** moves.
5. In the **Available Hosts** list, click on your host computer (it will be detected automatically).
   - *If your host does not appear automatically, click the **Gear** icon and enter your host's local IP address manually.*
6. Click **Start Streaming**.

---

## Step 4: Configure Games & Discord

Once streaming has started:

1. Open **Discord** or your game's **Audio Settings** on the Host machine.
2. Under **Input Device / Microphone**, select:
   - **Windows**: `CABLE Output (VB-Audio Virtual Cable)`
   - **macOS**: `BlackHole 2ch`
3. Speak into your client laptop's microphone and verify that your host app receives your voice loud and clear.

---

## Troubleshooting & FAQ

### The host computer is not showing up in the list
- **Same Network**: Ensure both computers are connected to the same local Wi-Fi router or Ethernet network.
- **Firewall Check**: On Windows, check Windows Defender Firewall to make sure MicStream is allowed on Private networks.
- **Manual IP Connection**: Find your host PC's local IP address (e.g. `192.168.1.150`), click the **Gear** icon in MicStream on the client, enter the IP, and connect directly.

### I can hear myself / others can't hear me in Discord
- Check that MicStream on the Host is outputting to `CABLE Input`.
- Check that Discord on the Host is listening to `CABLE Output`.
- Make sure you clicked **Start Listening** on the Host and **Start Streaming** on the Client.

### Microphone permission errors on macOS
- Open **System Settings** > **Privacy & Security** > **Microphone**.
- Ensure that **MicStream** is toggled ON.

### macOS Gatekeeper warning ("App is damaged and can't be opened" or "Unidentified developer")
- On modern macOS versions (Sonoma/Sequoia), downloaded unsigned binaries are placed in quarantine by Gatekeeper.
- Open **System Settings** > **Privacy & Security**, scroll to **Security**, and click **Open Anyway**.
- Alternatively, run the following command in Terminal to clear the quarantine flag:
  ```bash
  xattr -cr /Applications/MicStream.app
  ```

### Audio sounds choppy or stuttering
- Ensure your client device has a stable Wi-Fi or Ethernet connection.
- Click the **Gear** icon and verify the transport mode is set to **Opus Low-Delay** (recommended for wireless connections).
- In Advanced Settings, you can slightly increase the **Target Jitter Buffer** (e.g., from `5.0ms` to `10.0ms`) to absorb network fluctuations.
