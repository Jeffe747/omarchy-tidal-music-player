# Omarchy Tidal Music Player (`jaj.tidal`)

The dedicated Tidal music player for the [Omarchy](https://omarchy.org/) Linux desktop, built for Quickshell, Hyprland, and Wayland.

## Overview

`jaj.tidal` brings Tidal playback and library browsing into Omarchy as a native desktop feature. It replaces the need for a heavy Electron or browser-based player with a lean Quickshell interface backed by a headless Rust daemon. The shell handles presentation and interaction; the daemon handles authentication, catalog requests, playback, and desktop integration.

## Design Goals & Philosophy

- **Native Omarchy design language:** The interface follows `qs.Commons.Color` and `qs.Commons.Style`, responds live to theme changes, avoids foreign styling, and uses smooth, restrained animations.
- **Extreme efficiency and low footprint:** A lean Rust daemon communicates with Quickshell over a Unix domain socket. Headless `mpv` handles decoding and sends audio directly to PipeWire.
- **Bit-perfect audio fidelity:** Lossless FLAC playback preserves the delivered audio stream, including Hi-Res Lossless up to 24-bit / 192 kHz when available from the account and track.
- **Zero-configuration, one-command setup:** The plugin can bootstrap its matching release daemon binary on first launch. End users do not need build tools, and OAuth 2.0 device pairing completes through Tidal's device login flow.
- **Deep desktop and MPRIS2 interoperability:** The player registers as `org.mpris.MediaPlayer2.Tidal` and works with keyboard media keys, `playerctl`, and Omarchy's `omarchy.media` integration.

## What It Does

- **Status bar widget and hero card:** Shows now-playing status and artwork with playback controls, a volume slider, and a seekbar.
- **Library and catalog discovery:** Browse favorites and playlists, create playlists inline, search the catalog, explore artists and albums, and start Song Radio.
- **Track context actions:** Toggle favorites, add tracks to playlists, or open Song Radio for a track.
- **Desktop media control:** Integrates with MPRIS2 and `playerctl`, with PipeWire volume control and keyboard media keys.

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
| tidal-daemon (Rust; downloaded official release binary)                |
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
│   └── tidal-daemon     # Bundled UPX-compressed Linux x86-64 release daemon (< 800 KB)
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
        ├── log.rs       # Private, redacted persistent daemon diagnostics
        └── ipc.rs       # Unix domain socket communications
```

---

## Installation & Setup

### 1-Command Omarchy Installation

```sh
omarchy plugin add https://github.com/Jeffe747/omarchy-tidal-music-player --enable
```

On first launch, `Service.qml` downloads the matching official release binary
when no system, user-state, bundled, or development binary is available. The
daemon uses system runtime libraries; Omarchy/Quickshell, `mpv`, PipeWire,
`curl` or `wget`, and `xdg-open` remain runtime requirements.

### AUR Package Installation

```sh
yay -S tidal-daemon
```

### Building from Source

```sh
cargo build --release --manifest-path backend/Cargo.toml
```

### AUR Installation & Systemd Service

Install the daemon from the AUR with `yay -S tidal-daemon` or, on Omarchy,
`omarchy pkg aur add tidal-daemon`. Start it as a user service:

```sh
systemctl --user enable --now tidal-daemon.service
```

The package installs `tidal-daemon` to `/usr/bin` and its user unit. The daemon registers the `Tidal` MPRIS player. Standard tools can control it
and read its metadata:

```sh
playerctl -p Tidal play-pause
playerctl -p Tidal next
playerctl -p Tidal metadata
```

The standalone IPC server listens on the Unix domain socket
`$XDG_RUNTIME_DIR/tidal.sock`. Send one JSON command per line; for example,
with `socat` installed, `printf '{"command":"get_status"}\n' | socat - UNIX-CONNECT:"$XDG_RUNTIME_DIR/tidal.sock"`
requests current player status.

Session, settings, and daemon log files default to `~/.local/state/tidal/`.
For existing installations, if the matching file exists under
`~/.local/state/omarchy/tidal/` and no file exists at its new location, the
daemon continues using the legacy file. `TIDAL_SESSION_PATH`,
`TIDAL_SETTINGS_PATH`, and `TIDAL_LOG_PATH` can each override their file path;
`XDG_STATE_HOME` selects the state directory.

---

### Option 2: Local / Manual Installation

#### 1. Clone the Repository
```bash
git clone https://github.com/Jeffe747/omarchy-tidal-music-player.git
cd omarchy-tidal-music-player
```

The self-hydrating plugin downloads its daemon when installed without a local
binary. Developers changing the Rust backend need Rust (via [`mise`](DEVELOPMENT.md),
`omarchy pkg add rust`, or `rustup`) and can run `./scripts/build.sh`.

#### 2. Install the Standalone Plugin
```bash
./install.sh
```

The installer copies the plugin into `~/.config/omarchy/plugins/jaj.tidal`,
without Git metadata or build caches. Omarchy rejects symlinked plugin
directories. Run `./install.sh` again after editing the source.

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

The plugin includes the public OAuth client credentials for client ID
`4N3n6Q1x95LL5K7p`, so login and session refresh work without setting any
environment variables. Set `TIDAL_CLIENT_SECRET` only when using a custom
`TIDAL_CLIENT_ID`. Sessions issued for the previous client may require signing
in again.

Run `bin/tidal-daemon --probe <track_id>` with an authenticated session to print
track metadata, catalog and resolved audio quality, manifest MIME type, and a
60-character stream-location preview. The probe resolves the manifest without
starting `mpv` or playing audio, and cleans up any temporary DASH manifest.
Success confirms supported manifest resolution, not CDN access or audible
playback. Missing sessions, invalid arguments, API errors, and unsupported
manifests exit with status 1. Stream previews may contain signed URL data;
do not share them publicly.

After login, the popup automatically fetches all favorite tracks, newest first.
Click a favorite to request lossless playback through headless `mpv` and PipeWire;
the hero card and bar label show its artwork and metadata. Favorites can be
retried after errors; unsupported encrypted BTS streams produce an explicit
playback error. The daemon fetches the real country from `/v1/sessions` before
catalog/playback requests, caches it for the active token, and persists it in
the session file; it never substitutes `US`. Playback uses
`playbackinfopostpaywall`, retries the legacy `playbackinfo` endpoint once on
HTTP 404, and retries with `HIGH` if Tidal explicitly rejects the requested
audio quality. The displayed quality reflects the actual response.

Private diagnostics are appended to
`~/.local/state/tidal/daemon.log` (directory mode `0700`, file mode
`0600`). A log larger than 256 KiB is truncated at daemon startup. It records
IPC command names, API paths, HTTP failures with Tidal's `userMessage` and
`subStatus`, manifest MIME types, and mpv load results, without authorization
headers, tokens, or signed stream URLs. Country lookup failures are reported
instead of silently using the wrong region.

The verification gate covers favorites parsing, stream resolution fixtures, mpv startup and stale-socket cleanup, QML rendering and click dispatch, MPRIS integration, and the bundled daemon IPC smoke test.

---

## v1.0.0 User Guide

### Keyboard shortcuts

- `/` focuses the universal search field. Typing while the panel is focused also enters a search query.
- `Space` toggles playback when a text field is not focused.
- `Left` and `Right` seek backward or forward by five seconds when text input is not focused.
- `Up` and `Down` move the selection through the visible library or search results.
- `Enter` plays the selected track or opens the selected playlist; with no pending selection it toggles playback.
- `Esc` backs out in context order: closes the track menu, cancels playlist creation or logout confirmation, returns from exploration/playlist details, clears search, closes settings, then closes the panel.
- `Tab` switches to the adjacent Omarchy panel.

### Mouse controls and track menu

- On the Tidal bar icon, middle-click toggles play/pause, right-click skips to the next track, and the scroll wheel skips tracks. A normal click opens or closes the flyout.
- Use the volume slider to set PipeWire output volume. Its wheel adjusts volume in small steps; right-click toggles mute.
- Right-click a track row, artist link, or album link to open the track context menu. Choose **Favorite**, **Add to playlist** (then select a playlist in the inline picker), or **Go to song radio**.

### Library and discovery

The **Favorites** tab shows the favorite count, loading/error/empty states, and a retry action after a fetch error. **Playlists** shows the account playlists and an inline **+ Create playlist** flow. Opening a playlist shows its tracks and a filter for title or artist. Universal search is live and debounced across the catalog; selecting a track starts playback.

Artist links open dedicated artist top-track views; album links open album track views. **Song Radio** from a track context menu opens recommendations fetched through `GET /v1/tracks/{id}/radio`.

### Playback card and settings

The playback card shows artwork, track and artist/album details, the negotiated audio quality badge, favorite toggle, Chromecast action, seek bar, shuffle/repeat and transport controls, and a PipeWire volume slider. The settings overlay provides the audio quality ladder (**Hi-Res**, **Lossless**, **High AAC**), daemon connection status, and logout with a confirmation step. The quality badge reports the negotiated stream quality when Tidal falls back from the selected tier.

## License

MIT
