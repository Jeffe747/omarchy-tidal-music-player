# Omarchy Tidal Music Player - Architecture & Implementation Plan

This document outlines the complete architectural design, protocol research, binary size optimization strategies, UI/theming integration, and phased implementation roadmap for `omarchy-tidal-music-player`.

---

## 1. Feasibility & Technical Research

### 1.1 Authentication & Authorization
Tidal uses OAuth 2.0. For a desktop desktop/panel widget, two authentication pathways are feasible:

1. **OAuth 2.0 Device Authorization Grant (`link.tidal.com`) [Primary]**:
   - Ideal for a desktop status bar panel without requiring local port forwarding or browser redirect hurdles.
   - **Step 1:** The daemon requests a device code:
     `POST https://auth.tidal.com/v1/oauth2/device/authorization` with `client_id` and scopes (`r_usr w_usr`).
   - **Step 2:** Tidal returns:
     - `deviceCode`
     - `userCode` (e.g. `ABCD-1234`)
     - `verificationUriComplete` (`https://link.tidal.com/ABCD-1234`)
     - `expiresIn` (e.g. 300s)
     - `interval` (e.g. 5s)
   - **Step 3:** The plugin UI renders the user code, opens the URL via `xdg-open`, and polls:
     `POST https://auth.tidal.com/v1/oauth2/token`
   - **Step 4:** On approval, Tidal returns `access_token`, `refresh_token`, and `user_id`.
   - **Storage:** Persisted in `~/.local/state/omarchy/tidal/session.json` with secure permissions (`0600`).

2. **OAuth 2.0 PKCE Flow [Secondary/Fallback]**:
   - `https://login.tidal.com/authorize` with code challenge, catching the authorization code on a local loopback listener (`http://127.0.0.1:<port>/callback`).

---

### 1.2 Catalog & Metadata API
Tidal provides REST endpoints (`api.tidal.com/v1/` and `openapi.tidal.com/v2/`):
- **Search:** `GET /v1/search?query={query}&types=TRACKS,ALBUMS,ARTISTS,PLAYLISTS&limit=25`
- **User Playlists:** `GET /v1/users/{userId}/playlists`
- **User Favorites:** `GET /v1/users/{userId}/favorites/tracks`, `GET /v1/users/{userId}/favorites/albums`
- **Album Artwork:** High-resolution CDN `https://resources.tidal.com/images/{cover_uuid}/640x640.jpg`

---

### 1.3 Audio Streaming & DRM Reality
- **Playback Info Endpoint:**
  `GET https://api.tidal.com/v1/tracks/{trackId}/playbackinfo`
  - Parameters:
    - `audioquality`: `LOSSLESS` (16-bit / 44.1 kHz FLAC), `HI_RES_LOSSLESS` (24-bit / up to 192 kHz FLAC), `HIGH` (320 kbps AAC), `LOW` (96 kbps AAC).
    - `playbackmode`: `STREAM`
    - `assetpresentation`: `FULL`
- **Manifest Payload:**
  The API returns a base64-encoded manifest:
  1. `application/vnd.tidal.bts`:
     - Once decoded from base64, returns JSON containing direct HTTPS audio CDN URLs (`urls: ["https://..."]`, `mimeType: "audio/flac"`, `encryptionType: "NONE"`).
     - **No DRM encryption** on standard FLAC and AAC streams.
  2. `application/dash+xml` (MPEG-DASH):
     - Used for Hi-Res Lossless streams.
     - Once decoded from base64, provides an XML MPEG-DASH MPD manifest referencing unencrypted FLAC audio chunks.
- **Audio Output Pipeline:**
  - Omarchy runs **PipeWire** natively.
  - Rather than embedding a massive custom DASH demuxer and decoder inside our binary, we drive **headless `mpv`** (already installed in Omarchy at `/usr/bin/mpv 0.41.0`) over Unix domain socket IPC (`--input-ipc-server`):
    - Supports MPEG-DASH MPD manifests and FLAC streams out of the box.
    - Provides gapless playback, buffer caching, volume normalization (ReplayGain), seeking, sample rate switching up to 192 kHz, and PipeWire output.
    - Zero need for Widevine on standard/Hi-Res FLAC audio.

---

### 1.4 MPRIS D-Bus Integration
By implementing the standard `org.mpris.MediaPlayer2.Tidal` interface on D-Bus:
- Omarchy's existing built-in **`omarchy.media`** bar widget and **`omarchy.audio`** volume panel automatically recognize the player.
- System hardware media keys (Play/Pause, Next, Previous) work instantly through Hyprland keybindings.

---

## 2. Language Selection & Architecture

| Language | Binary Size | Startup & Memory | Audio & Network Ecosystem | Recommendation |
| :--- | :--- | :--- | :--- | :--- |
| **Rust** | **~1.5 – 2.5 MB** (stripped & LTO) | < 10 ms startup, ~8–12 MB RAM | Outstanding (`ureq`, `serde`, `zbus`, `libmpv`/IPC, `symphonia`) | **Primary Choice (Selected)** |
| **C / C++** | **~200 – 400 KB** (stripped) | < 5 ms startup, ~5 MB RAM | Native `libcurl`, `libmpv`, `cJSON`, `systemd-bus` | Smallest binary, but higher dev/maintenance cost |
| **Go** | **~8 – 15 MB** (stripped) | ~20 ms startup, ~25 MB RAM | GC runtime overhead; Cgo needed for native audio/D-Bus | Not recommended for small binaries |
| **Python** | N/A (scripts only) | 150–300 ms, ~50 MB RAM | `tidalapi` exists, but Arch `PEP 668` venv issues & high latency | Prototype only |

### Choice: **Rust**
Rust provides the optimal combination of memory safety, zero-cost abstractions, predictable low latency, and a compact standalone binary without garbage collection overhead.

---

## 3. Binary Size Minimization Strategy

To keep the daemon binary as small as possible:

### 3.1 Cargo Release Profile (`Cargo.toml`)
```toml
[profile.release]
opt-level = "z"        # Optimize aggressively for binary size
lto = true             # Full link-time optimization across all dependencies
codegen-units = 1      # Maximize inlining and cross-crate dead code elimination
panic = "abort"        # Strip stack unwinding tables (.eh_frame) and landing pads
strip = true           # Automatically strip symbols and debuginfo
incremental = false
```

### 3.2 Dependency Selection
- **HTTP Client:** Use `ureq` (lightweight sync HTTP) with `rustls` (or dynamically link against system `libcurl`) instead of full `reqwest` (which pulls in `hyper`, `tokio`, OpenSSL, etc.).
- **Async/Runtime:** Avoid a monolithic multi-threaded runtime where simple threads and channels suffice, or use a minimal `tokio` feature set.
- **D-Bus:** Use `zbus` with default features disabled except core server modules.
- **Audio Decoding:** Do not bundle static FFmpeg or GStreamer libraries. Coordinate playback through system `mpv` over Unix IPC.

### 3.3 Post-Processing & Compression
- **ELF Stripping:**
  ```bash
  strip --strip-all --remove-section=.comment --remove-section=.note* target/release/tidal-daemon
  ```
- **UPX Compression (Optional):**
  ```bash
  upx --best --lzma target/release/tidal-daemon
  ```
  Reduces a ~2.2 MB binary down to **~650 KB – 800 KB**.

---

## 4. Omarchy UI & System Theme Integration

Omarchy shell plugins run inside the single long-lived `omarchy-shell` Quickshell host process.

### 4.1 System Theme Reactive Styling (`qs.Commons.Color`)
The UI binds dynamically to Omarchy's color singleton:
- `Color.foreground` / `Color.text`: Primary labels and iconography.
- `Color.background`: Flyout background.
- `Color.accent`: Active states, seek bar fill, playing indicator, Hi-Res badge.
- `Color.muted`: Secondary text, track metadata, album names, durations.
- `Color.urgent`: Error states and disconnect alerts.

### 4.2 Standard Omarchy Components (`qs.Ui`)
- `Panel`: Base widget handling open/close/toggle IPC events.
- `BarIconButton`: Bar icon with hover styling and tooltip.
- `KeyboardPanel`: Floating flyout window with focus grabbing and outside-click dismissal.
- `PanelSlider`: Smooth seek bar and volume controller.
- `TextField`: Search bar for searching Tidal tracks/playlists.
- `PanelSectionHeader` & `PanelSeparator`: Visual section division matching system panels.

---

## 5. Complete System Architecture

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
|  - src/mpris.rs    : D-Bus MPRIS server (org.mpris.MediaPlayer2.Tidal) |
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

---

## 6. Phased Implementation Roadmap

### Phase 1: Daemon Foundation & Authentication (Device Flow)
- Set up Rust project with release profile optimizations.
- Implement `src/auth.rs`:
  - Request device code from `https://auth.tidal.com/v1/oauth2/device/authorization`.
  - Display user code / verification URL.
  - Poll token endpoint and persist credentials in `~/.local/state/omarchy/tidal/session.json`.
  - Handle token refresh.

### Phase 2: Catalog API & Manifest Parsing
- Implement `src/api.rs`:
  - Search tracks, albums, artists, playlists.
  - Fetch user playlists and favorite tracks.
- Implement `src/playback.rs`:
  - Call `/v1/tracks/{id}/playbackinfo`.
  - Parse `application/vnd.tidal.bts` (direct HTTPS unencrypted stream URLs).
  - Parse `application/dash+xml` (MPEG-DASH manifests for Hi-Res FLAC).

### Phase 3: Playback Engine & MPRIS Integration
- Spawn headless `mpv` background runner with PipeWire audio output.
- Control playback, volume, pause/play, seek, and track queue over mpv IPC socket.
- Expose `org.mpris.MediaPlayer2.Tidal` on D-Bus via `zbus` for desktop media controls.

### Phase 4: Quickshell UI & Omarchy Theming
- Build `manifest.json`, `Service.qml`, and `BarWidget.qml`.
- Implement pairing UI for unauthenticated state.
- Implement player card with album art, track details, seek slider, and transport controls.
- Implement search view with live query results.
- Verify live theme switching with `omarchy theme set`.

### Phase 5: Packaging & Installation
- Provide `install.sh` to link into `~/.config/omarchy/plugins/jaj.tidal`.
- Provide build script and binary verification.
