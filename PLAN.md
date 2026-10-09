# Omarchy Tidal Music Player - Architecture & Implementation Plan

This document outlines the complete architectural design, protocol research, binary size optimization strategies, UI/theming integration, and phased implementation roadmap for `omarchy-tidal-music-player`.

---

## 1. Feasibility & Technical Research

### 1.1 Authentication & Authorization
Tidal uses OAuth 2.0. For a desktop desktop/panel widget, two authentication pathways are feasible:

- **Built-in public client credentials:** The daemon defaults to the public client ID `4N3n6Q1x95LL5K7p` and includes a zero-configuration built-in token fallback, so users can authenticate without supplying client credentials. `TIDAL_CLIENT_SECRET` is an optional environment override for deployments using a custom client ID (`TIDAL_CLIENT_ID`).

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
  `GET https://api.tidal.com/v1/tracks/{trackId}/playbackinfopostpaywall`
  - If the modern endpoint returns HTTP 404, retry the legacy `GET /v1/tracks/{trackId}/playbackinfo` endpoint.
  - Parameters:
    - `audioquality`: `LOSSLESS` (16-bit / 44.1 kHz FLAC), `HI_RES_LOSSLESS` (24-bit / up to 192 kHz FLAC), `HIGH` (320 kbps AAC), `LOW` (96 kbps AAC).
    - `playbackmode`: `STREAM`
    - `assetpresentation`: `FULL`
    - `immersiveaudio=false` to request a stereo stream.
  - **Quality fallback:** Start with the catalog's preferred quality, then try `LOSSLESS`, `HIGH`, and `LOW` in order when Tidal reports `subStatus=4005` (`Asset is not ready for playback`).
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

## 6. Implementation Progression Milestones

Milestones 0–6 are complete, as recorded in `PROGRESS.md`. Milestone 7 is
queued. Runtime checks that require a local Wayland/Quickshell session are
called out below; live Rimegale verification is recorded where available.

### Milestone 0: Foundation, Scaffolding & Agent Gates
- **Status: Complete.** Plugin manifest, QML and Rust scaffolding, verification scripts, Omarchy compliance validator, agent contract, and architecture plan are in place.

### Milestone 1: Authentication & Session Management (Login)
- **Status: Complete.** Device authorization, token polling and refresh, secure session persistence, IPC, and connected-state UI are implemented and verified.
- **Objective:** Enable frictionless OAuth 2.0 Device Flow login via `link.tidal.com` directly from the bar flyout.
- **Backend:**
  - Request device code (`userCode`, `verificationUriComplete`, `deviceCode`, `interval`, `expiresIn`).
  - Background polling worker for token issuance with timeout and error handling.
  - Secure credential storage in `~/.local/state/omarchy/tidal/session.json` (`0600` permissions).
  - Token refresh management.
- **IPC Protocol:**
  - Commands: `start_auth`, `get_auth_status`, `logout`.
  - Push events: `auth_code_ready`, `auth_success`, `auth_expired`.
- **Quickshell UI (`BarWidget.qml`):**
  - Unauthenticated pairing card displaying user code and "Open link.tidal.com in Browser" button (`xdg-open`).
  - Automatic transition to "Connected" state upon browser confirmation.

### Milestone 2: Favorites List & Core Audio Playback
- **Status: Completed and verified live on Rimegale**, including authenticated stream resolution and PipeWire audio playback.
- **Objective:** Fetch user favorite tracks and stream lossless audio through PipeWire.
- **Backend:**
  - Fetch favorites: `GET /v1/users/{userId}/favorites/tracks` (titles, artists, albums, durations, artwork IDs).
  - Stream resolver: `GET /v1/tracks/{trackId}/playbackinfopostpaywall` with legacy `/playbackinfo` fallback on 404; parse `application/vnd.tidal.bts` (direct HTTPS FLAC/AAC) and `application/dash+xml` (MPEG-DASH), with catalog-quality -> `LOSSLESS` -> `HIGH` -> `LOW` fallback for `subStatus=4005`.
  - Audio output: Headless `mpv` background process managed via Unix IPC (`--idle=yes --no-video --ao=pipewire`).
- **IPC Protocol:**
  - Commands: `get_favorites`, `play_track { track_id }`.
  - Push events: `favorites_loaded`, `playback_started`, `track_metadata`.
- **Quickshell UI:**
  - Scrollable favorites list in flyout card.
  - Track selection to trigger streaming.
  - Now-playing hero card with track title, artist name, and album artwork.

### Milestone 3: Interactive Playback Controls, Seeking & Auto-Advance
- **Status: Complete.** Playback controls, position updates, queue navigation, and EOF auto-advance are implemented and covered by automated tests. QML runtime interaction checks remain pending without a Wayland/Quickshell session.
- **Objective:** Interactive seek bar, transport controls, and automatic track queue advance.
- **Backend:**
  - mpv IPC control: `pause`, `resume`, `toggle_pause`, `seek_absolute(seconds)`, `set_volume(pct)`.
  - Progress emitter: Poll mpv `time-pos` at 250ms intervals and push to QML.
  - Queue auto-advance: Listen for mpv `eof-reached` to automatically play the next song in the favorites queue.
- **Quickshell UI:**
  - Smooth seek bar (`PanelSlider`) with `mm:ss` position and duration labels.
  - Transport buttons: Previous (`⏮`), Play/Pause toggle (`▶ / ⏸`), Next (`⏭`).
  - Bar button shortcuts: Left-click toggles panel; right-click toggles play/pause without opening panel.

### Milestone 4: MPRIS D-Bus & Desktop Integration
- **Status: Complete.** Live D-Bus introspection, `playerctl` controls/metadata/seeking, Hyprland media keybinding, and Omarchy media service integration were verified on Rimegale.
- **Objective:** Native desktop media control across Hyprland and Omarchy.
- **Backend:**
  - Register `org.mpris.MediaPlayer2.Tidal` on the session D-Bus.
  - Implement `org.mpris.MediaPlayer2.Player` interface (`PlaybackStatus`, `Metadata`, `PlayPause`, `Next`, `Previous`, `Seek`).
- **Desktop Integration:**
  - Keyboard media keys (`XF86AudioPlay`, etc.) control playback globally.
  - Omarchy's built-in `omarchy.media` bar widget automatically recognizes Tidal.
  - Omarchy's `omarchy.audio` volume flyout lists Tidal's PipeWire stream.

### Milestone 5: Catalog Search & Discovery
- **Status: Complete.** Authenticated search, debounced UI, and one-click playback are implemented; live searches and playback of a result were verified.
- **Objective:** Search any track, artist, album, or playlist from the flyout.
- **Backend:**
  - Query Tidal's search API: `GET /v1/search?query={q}&types=TRACKS,ALBUMS,PLAYLISTS&limit=20`.
- **Quickshell UI:**
  - Debounced search bar (`TextField`) at the bottom of the flyout.
  - Instant results view with single-click playback.

### Milestone 6: User Playlists & Audio Quality Tiers
- **Status: Complete.** Playlist loading, track queues, persistent quality selection, fallback negotiation, DASH handling, and themed quality UI are implemented and verified. Live `HI_RES_LOSSLESS` negotiation fell back to playable LOSSLESS; actual 24-bit playback remains unverified.
- **Objective:** Access custom playlists and toggle audio quality tiers (Hi-Res Lossless vs High AAC).
- **Backend:**
  - Endpoints for `GET /v1/users/{userId}/playlists` and `GET /v1/playlists/{uuid}/tracks`.
  - Streaming quality preferences in settings: `HI_RES_LOSSLESS` (up to 24-bit / 192 kHz FLAC), `LOSSLESS` (16-bit / 44.1 kHz FLAC), or `HIGH` (320 kbps AAC).
- **Quickshell UI:**
  - Tabbed library view (Favorites vs. Playlists).
  - Audio quality badge (`HI-RES FLAC`, `LOSSLESS`, `HIGH`) styled with `Color.accent`.
  - Settings dropdown to select streaming quality.

### Milestone 7: Binary Minimization, Theme Verification & Final Polish
- **Objective:** Final optimization, automated test suite, and theme verification.
- **Optimization:** Release build with LTO, size-stripping, and UPX compression (< 1.5 MB uncompressed, < 800 KB compressed).
- **Theme Testing:** Live theme switching across all stock Omarchy themes (`catppuccin`, `tokyo-night`, `nord`, etc.) with zero color clipping or restart requirements.

---

## 7. Verification Framework & Pre-Milestone Gates

Before advancing to each subsequent milestone, the code must pass the corresponding verification gates:

### Global Verification Suite
The verification script `scripts/verify.sh` runs the following automated checks:

1. **Automated Unit Tests (`cargo test`)**:
   - Serialization and deserialization of tokens, credentials, and state.
   - Parsing of Tidal JSON API responses and error envelopes.
   - Base64 manifest decoding and stream URL extraction (BTS & DASH).
   - IPC command parsing and state message validation.
2. **Binary Size & Budget Audit (`scripts/verify-size.sh`)**:
   - Release binary size must remain strictly under the **2.5 MB** budget (target: < 1.8 MB).
   - Stripped symbols check: verify `.comment`, `.note`, and debug symbols are removed.
3. **Omarchy Plugin & Manifest Validation**:
   - `omarchy plugin validate .` must exit with return code `0`.
   - QML component syntax and property bindings validation.

### Milestone-by-Milestone Verification Gates

| Milestone | Automated Tests | Integration & Hardware Checks | Pre-requisite Gate to Proceed |
| :--- | :--- | :--- | :--- |
| **M1: Login** | Unit tests for `auth.rs` (device code deserialization, token poll parser, expiry calculation). | CLI test runner (`--test-auth`) initiates device flow; visiting `link.tidal.com` issues tokens to `~/.local/state/omarchy/tidal/session.json`. | Tokens successfully acquired, stored, and auto-refreshed. UI shows "Connected". |
| **M2: Favorites & Playback** | Unit tests for `api.rs` (favorites parser) and `playback.rs` (manifest decoding). | Verified live on Rimegale: authenticated stream playback through headless `mpv` and PipeWire. | Complete; clean audio playback from user favorites with artwork and title in UI. |
| **M3: Controls & Seek** | Unit tests for time formatting, position bounds clamping, and queue index logic. | Interactive slider seek latency < 100ms; simulated `eof-reached` auto-advances to next track. | Pause, resume, seek, and auto-advance verified with zero stutter. |
| **M4: MPRIS & Media Keys** | D-Bus interface schema and property compliance tests. | `playerctl status` and `playerctl metadata` report track details; keyboard media keys control player. | Global keyboard shortcuts and Omarchy desktop widgets control Tidal. |
| **M5: Search** | Unit tests for search query escaping and result structure parsing. | Debounced search queries complete in < 500ms; one-click play from search results verified. | Search returns accurate results and immediately plays selected song. |
| **M6: Playlists & Quality** | Unit tests for playlist track fetch and audio quality parameter negotiation. | Audio stream inspect: verify 24/96 or 24/192 FLAC stream negotiated when `HI_RES_LOSSLESS` is selected. | Custom playlists playable; quality badge matches stream parameters. |
| **M7: Themes & Polish** | Full test suite passes; binary budget check passes (< 1.5 MB). | Theme switching test across 3+ Omarchy themes (`omarchy theme set`); network drop recovery test. | All tests pass, binary is lightweight, UI matches all themes seamlessly. |
