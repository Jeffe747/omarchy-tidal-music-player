# Omarchy Tidal Music Player (`jaj.tidal`)

> High-fidelity Tidal music streaming, library search, and player controls tailored for the [Omarchy](https://omarchy.org/) desktop with Quickshell and Hyprland.

---

## Highlights

- **Lossless & Hi-Res Audio:** Streams unencrypted FLAC (16-bit / 44.1 kHz) and Hi-Res Lossless (up to 24-bit / 192 kHz) via PipeWire and headless `mpv`.
- **System Theme Integration:** 100% reactive to Omarchy themes (`catppuccin`, `tokyo-night`, `nord`, etc.) via `qs.Commons.Color`.
- **Zero-Friction Device Login:** Seamless OAuth 2.0 device pairing via `link.tidal.com` directly from the bar popup.
- **Compact Standalone Binary:** Lean Rust backend daemon (`tidal-daemon`) built with LTO, size optimization (`opt-level = "z"`), and symbol stripping (~1.5 MB).
- **MPRIS Desktop Integration:** Registers `org.mpris.MediaPlayer2.Tidal` on D-Bus, seamlessly connecting to Omarchy's `omarchy.media` widget and keyboard media keys.

---

## Architecture Overview

```
+-------------------------------------------------------------------------+
| Omarchy Shell (Quickshell / QML)                                        |
|  - BarWidget.qml : Bar status button, now-playing badge, flyout toggle  |
|  - Service.qml   : Background IPC client & reactive state store         |
|  - Flyout UI     : Track info, seekbar, controls, search, auth pairing  |
+------------------------------------+------------------------------------+
                                     | JSON-RPC over Unix Socket
                                     v
+-------------------------------------------------------------------------+
| tidal-daemon (Rust, ~1.5 MB)                                            |
|  - src/auth.rs     : OAuth 2.0 Device Flow (link.tidal.com)             |
|  - src/api.rs      : Tidal REST client (catalog, search, playlists)     |
|  - src/playback.rs : Playback info manifest parser & mpv coordinator    |
|  - src/ipc.rs      : Unix socket server (/run/user/1000/tidal.sock)     |
+------------------+------------------------------------+-----------------+
                   |                                    |
                   v IPC                                v HTTPS
+----------------------------------+   +----------------------------------+
| mpv (Headless Playback Engine)   |   | Tidal API & Audio CDN            |
|  - MPEG-DASH / FLAC decoding     |   |  - auth.tidal.com                |
|  - PipeWire audio sink output    |   |  - api.tidal.com                 |
|  - Bit-perfect 24/192 support    |   |  - resources.tidal.com           |
+----------------------------------+   +----------------------------------+
```

For the comprehensive research report, binary tuning parameters, and implementation phases, see **[`PLAN.md`](PLAN.md)**.

---

## Repository Structure

```
.
├── PLAN.md              # Complete architecture, protocol research & implementation plan
├── README.md            # Project documentation and quick start guide
├── manifest.json        # Omarchy plugin descriptor (jaj.tidal)
├── BarWidget.qml        # Top bar widget button & popup player card
├── Service.qml          # Quickshell background IPC service
├── install.sh           # Plugin installer and shell registration helper
├── scripts/
│   └── build.sh         # Release build script with binary size optimizations
└── backend/
    ├── Cargo.toml       # Rust crate manifest with size profile
    └── src/
        ├── main.rs      # Daemon entry point & IPC loop
        ├── auth.rs      # OAuth 2.0 Device Authorization flow
        ├── api.rs       # Tidal REST API catalog client
        ├── playback.rs  # Manifest parser & mpv IPC controller
        └── ipc.rs       # Unix domain socket communications
```

---

## Installation & Setup

### 1. Install the Plugin into Omarchy Shell
```bash
./install.sh
```

### 2. Build the Backend Daemon
Requires Rust (can be installed via `omarchy pkg add rust` or rustup):
```bash
./scripts/build.sh
```

### 3. Enable in Status Bar
Add `"jaj.tidal"` to your status bar layout in `~/.config/omarchy/shell.json`:
```json
{
  "bar": {
    "layout": {
      "right": [
        { "id": "jaj.tidal" }
      ]
    }
  }
}
```
Or run:
```bash
omarchy plugin enable jaj.tidal
```

---

## License

MIT
