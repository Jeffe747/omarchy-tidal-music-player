# Omarchy Tidal Music Player (`jaj.tidal`)

> High-fidelity Tidal music streaming, library search, and player controls tailored for the [Omarchy](https://omarchy.org/) desktop with Quickshell and Hyprland.

---

## Highlights

- **Lossless & Hi-Res Audio:** Streams unencrypted FLAC (16-bit / 44.1 kHz) and Hi-Res Lossless (up to 24-bit / 192 kHz) via PipeWire and headless `mpv`.
- **System Theme Integration:** 100% reactive to Omarchy themes (`catppuccin`, `tokyo-night`, `nord`, etc.) via `qs.Commons.Color`.
- **Zero-Friction Device Login:** Seamless OAuth 2.0 device pairing via `link.tidal.com` directly from the bar popup.
- **Bundled Standalone Binary:** Tracked, executable `bin/tidal-daemon`, stripped and limited to 1.8 MB; installation needs no Rust toolchain or compilation.
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
For the live milestone progress tracker and task checklist, see **[`PROGRESS.md`](PROGRESS.md)** (or run `./scripts/status.sh`).

---

## Repository Structure

```
.
├── AGENTS.md            # Mandatory agent operating contract and gates
├── PLAN.md              # Complete architecture, protocol research & implementation plan
├── PROGRESS.md          # Live milestone roadmap, task checklist & activity log
├── README.md            # Project documentation and quick start guide
├── DEVELOPMENT.md       # Development requirements, toolchains & workflow guide
├── mise.toml            # mise-en-place toolchain definition (pins rust = "stable")
├── manifest.json        # Omarchy plugin descriptor (jaj.tidal)
├── BarWidget.qml        # Top bar widget button & popup player card
├── Service.qml          # Quickshell background IPC service
├── install.sh           # Plugin installer and shell registration helper
├── bin/
│   └── tidal-daemon     # Bundled stripped Linux x86-64 release daemon (<= 1.8 MB)
├── scripts/
│   ├── build.sh         # Release build script with binary size optimizations
│   ├── status.sh        # Terminal dashboard for current progress
│   ├── verify.sh        # Global test & verification runner
│   ├── verify-size.sh   # Binary size & symbol stripping auditor
│   └── verify-omarchy-compliance.sh # Omarchy official compliance auditor
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

### Option 1: Official Omarchy CLI (Recommended)
Install and enable the plugin directly using the official `omarchy` CLI:
```bash
omarchy plugin add https://github.com/Jeffe747/omarchy-tidal-music-player.git --enable
```

The repository includes `bin/tidal-daemon` for **zero-dependency out-of-the-box
installation on a compatible Linux x86-64 Omarchy system**: no Cargo, Rust, or
build step is required. The daemon uses the system's runtime libraries;
Omarchy/Quickshell, `mpv`, PipeWire, and `xdg-open` remain runtime requirements.
`Service.qml` prefers the executable bundle and falls back to
`backend/target/release/tidal-daemon` only in development checkouts.

---

### Option 2: Local / Manual Installation

#### 1. Clone the Repository
```bash
git clone https://github.com/Jeffe747/omarchy-tidal-music-player.git
cd omarchy-tidal-music-player
```

The bundled daemon is ready to use. Only developers changing the Rust backend
need Rust (via [`mise`](DEVELOPMENT.md), `omarchy pkg add rust`, or `rustup`) and
`./scripts/build.sh`, which also refreshes `bin/tidal-daemon`.

#### 2. Install the Standalone Plugin
```bash
./install.sh
```

The installer requires the executable, stripped bundle to be at most
1,800,000 bytes, then copies the plugin and `bin/tidal-daemon` into
`~/.config/omarchy/plugins/jaj.tidal`, without Git metadata or build caches.
Omarchy rejects symlinked plugin directories. Run `./install.sh` again after
editing the source or rebuilding the daemon.
Contributors must commit the refreshed bundle alongside backend changes.

> [!TIP]
> For complete development toolchains, system package setup, and dependencies, see **[`DEVELOPMENT.md`](DEVELOPMENT.md)**.

#### 3. Enable in Status Bar
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

## Development & Contributing

For detailed setup instructions, toolchain configuration (`mise`, `rustup`), runtime dependencies (`mpv`, `pipewire`, `quickshell`), and verification scripts, see **[`DEVELOPMENT.md`](DEVELOPMENT.md)**.

### Quick Verification Run
Before submitting changes or marking milestones complete, run the full verification gate:
```bash
./scripts/verify.sh
```

- **Run unit tests:** `cargo test --manifest-path backend/Cargo.toml`
- **Verify Omarchy compliance:** `./scripts/verify-omarchy-compliance.sh`
- **Audit binary size & symbols:** `./scripts/verify-size.sh`
- **Track roadmap progress:** `./scripts/status.sh`

The verification gate also loads the widget in a separate Quickshell instance
when Omarchy is installed and a Wayland session is available. It checks the bar
slot's visibility and dimensions, shared service lookup, and panel controls.
Run `WAYLAND_DISPLAY=wayland-1 ./scripts/verify-qml.sh` to run this check alone.

If the bar reports an old QML loading error after an update, unlock the desktop
and run `omarchy restart shell`, then inspect
`WAYLAND_DISPLAY=wayland-1 quickshell log -p /usr/share/omarchy/shell`.
The Tidal icon stays visible before login and when its service is unavailable;
widgets use the shell's shared service rather than starting a second client.
Commands issued while the daemon connects are delivered after connection.

### Favorites & Playback (M2)

After login, the popup automatically fetches all favorite tracks, newest first.
Click a favorite to request lossless playback through headless `mpv` and PipeWire;
the hero card and bar label show its artwork and metadata. Favorites can be
retried after errors; unsupported encrypted BTS streams produce an explicit
playback error. Session country codes are used when available, otherwise `US`.

The local gate covers favorites parsing, stream resolution with fixtures, real
mpv startup/stale-socket recovery/cleanup, and QML rendering and click dispatch.
Authenticated Tidal playback and the live PipeWire/`pw-cli` audio check remain
pending on the remote laptop; M2 is not yet a fully passed milestone. Continuous
position updates, queue navigation/auto-advance, search, and MPRIS remain later
milestones.

---

## License

MIT
