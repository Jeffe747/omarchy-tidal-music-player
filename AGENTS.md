# AGENTS.md — Agent Gate & Operating Contract

Welcome to **`omarchy-tidal-music-player`** (`jaj.tidal`).

Any AI agent (Antigravity, Claude, Codex, Copilot, etc.) working on this repository **must read and adhere to this document** before implementing, modifying, or refactoring code.

---

## 1. Project Mission & Architecture

This project is a first-class, lightweight **Tidal music streaming plugin for the [Omarchy](https://omarchy.org/) desktop shell** (Arch Linux + Hyprland + Quickshell).

### System Architecture
```
+-------------------------------------------------------------------------+
| Omarchy Shell (Quickshell / QML)                                        |
|  - BarWidget.qml : Bar status button, now-playing badge, popup card     |
|  - Service.qml   : Background IPC client & reactive state properties    |
+------------------------------------+------------------------------------+
                                     | JSON-RPC over Unix Socket ($XDG_RUNTIME_DIR/tidal.sock)
                                     v
+-------------------------------------------------------------------------+
| tidal-daemon (Rust, < 1.8 MB stripped)                                  |
|  - src/auth.rs     : OAuth 2.0 Device Flow (link.tidal.com)             |
|  - src/api.rs      : Tidal REST API client (catalog, search, playlists) |
|  - src/playback.rs : Manifest parser & mpv IPC coordinator              |
|  - src/ipc.rs      : Unix socket server speaking JSON-RPC to Quickshell |
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

## 2. Mandatory Agent Rules & Agreements

1. **Cleanliness & State Tracking**:
   - Track every temporary file, socket, scratch script, and experimental config created during development.
   - Clean up after experiments. Never leave orphaned background processes (e.g. dangling `mpv` or `tidal-daemon` test instances) running.
   - If any untidy state is left behind, report it explicitly to the user.
2. **The Verification Gate Rule**:
   - **NEVER** mark a milestone complete or move to the next milestone without running `./scripts/verify.sh` and verifying that all tests and checks pass.
3. **Binary Size Discipline**:
   - The compiled release binary `tidal-daemon` must strictly remain under **2.5 MB** (`MAX_SIZE_KB=2560`), with an ideal target of **< 1.8 MB**.
   - Do **NOT** pull in bloated dependencies (e.g., full `reqwest` with OpenSSL, massive web runtimes, or bundled FFmpeg libraries).
   - Use `ureq`, `serde`, and coordinate with system `mpv` for audio decoding.
4. **Omarchy Theming & Style Compliance**:
   - **NEVER** hardcode hex colors or arbitrary margins in QML.
   - Always bind to `qs.Commons.Color` (`Color.foreground`, `Color.background`, `Color.accent`, `Color.muted`, `Color.urgent`).
   - Always use `qs.Commons.Style` (`Style.space()`, `Style.radiusMedium`, `Style.font.*`).
   - The UI must dynamically react to `omarchy theme set <theme>` with zero restarts.
5. **Manifest Integrity**:
   - Any change to `manifest.json` must be verified with `omarchy plugin validate .`.

---

## 3. Standard Commands & Verification

Every agent must use these standard project scripts:

| Command | Action |
| :--- | :--- |
| `./scripts/build.sh` | Compiles `tidal-daemon` in release mode with size optimizations and stripping. |
| `(cd backend && cargo test)` | Runs all unit tests. |
| `./scripts/verify-size.sh` | Audits release binary size (< 2.5 MB) and verifies symbol stripping. |
| `./scripts/verify.sh` | **Full gate runner:** runs manifest validation, unit tests, and binary size audit. |
| `./install.sh` | Links plugin to `~/.config/omarchy/plugins/jaj.tidal` and triggers shell rescan. |

---

## 4. Milestone Progression & Current Status

Implementation is strictly phased into 7 milestones:

| # | Milestone | Status | Gate Pre-requisite to Advance |
| :---: | :--- | :---: | :--- |
| **M1** | **Authentication & Session Management (Login)** | 🟡 **Next** | Unit tests pass for `auth.rs`; device code generated; tokens persisted to `~/.local/state/omarchy/tidal/session.json`; UI shows "Connected". |
| **M2** | **Favorites List & Core Audio Playback** | ⚪ Queued | Favorites API parsed; unencrypted FLAC manifest resolved; headless `mpv` streams audio to PipeWire; now-playing artwork/title renders in UI. |
| **M3** | **Interactive Controls, Seeking & Auto-Advance** | ⚪ Queued | Seek slider latency < 100ms; pause/play/next transport works; EOF auto-advances to next favorite; right-click toggles on bar. |
| **M4** | **MPRIS D-Bus & Desktop Integration** | ⚪ Queued | `org.mpris.MediaPlayer2.Tidal` on session bus; media keys and `playerctl` control playback; Omarchy media widgets sync. |
| **M5** | **Catalog Search & Discovery** | ⚪ Queued | Debounced search queries complete in < 500ms; one-click play from search results. |
| **M6** | **User Playlists & Audio Quality Tiers** | ⚪ Queued | Custom playlists load; quality badge reflects Hi-Res FLAC / Lossless / High AAC. |
| **M7** | **Binary Minimization & Theme Polish** | ⚪ Queued | Full test suite passes; binary budget < 1.5 MB (< 800 KB with UPX); flawless live theme switching across Omarchy themes. |

---

## 5. Milestone Verification Gates

Before an agent claims a milestone as complete, it **must** run the corresponding verification checklist:

### Gate M1: Login Verification
```bash
./scripts/verify.sh
```
- [ ] `auth::tests::test_device_auth_info_deserialization` passes.
- [ ] `auth::tests::test_session_serialization_roundtrip` passes.
- [ ] Running daemon or test harness receives user code from Tidal.
- [ ] Confirming on `link.tidal.com` persists session to `~/.local/state/omarchy/tidal/session.json`.
- [ ] Bar flyout updates from pairing prompt to "Connected" without restarting the shell.
- [ ] Binary size is < 2.5 MB.

### Gate M2: Favorites & Playback Verification
```bash
./scripts/verify.sh
```
- [ ] `playback::tests::test_parse_bts_manifest` passes.
- [ ] Unit tests for favorites JSON parsing pass.
- [ ] Daemon resolves direct stream URL from `playbackinfo`.
- [ ] Audio stream plays through PipeWire without distortion.
- [ ] Album artwork and track metadata render on `BarWidget.qml`.

### Gate M3: Controls & Seeking Verification
- [ ] Seeking through `PanelSlider` repositions stream within 100ms.
- [ ] Transport buttons (`⏮`, `▶ / ⏸`, `⏭`) respond correctly.
- [ ] Reaching the end of a song automatically triggers the next song.
- [ ] Right-clicking the status bar button toggles play/pause.

### Gate M4: MPRIS Verification
- [ ] `busctl --user introspect org.mpris.MediaPlayer2.Tidal /org/mpris/MediaPlayer2` returns valid properties.
- [ ] `playerctl status` and `playerctl metadata` report correct track info.
- [ ] Keyboard media keys (`XF86AudioPlay`) control playback.

### Gate M5: Search Verification
- [ ] Queries typed in `TextField` execute debounced search.
- [ ] Clicking a search result immediately buffers and plays the track.

### Gate M6: Playlists & Audio Quality Verification
- [ ] User playlists load correctly.
- [ ] Hi-Res Lossless streams negotiate 24-bit audio when selected.
- [ ] Quality badge renders accurately with `Color.accent`.

### Gate M7: Final Polish & Release
- [ ] Run `omarchy theme set catppuccin`, `tokyo-night`, `everforest` — UI recolors dynamically with zero visual glitches.
- [ ] Run `./scripts/build.sh` and `./scripts/verify-size.sh`.
- [ ] Confirm UPX compression shrinks binary to < 800 KB.

---

## 6. Code Style & Architecture Conventions

- **Rust (`backend/`):**
  - Idiomatic Rust 2021 edition.
  - Zero `unwrap()` in production paths — handle errors with `Result<T, String>` or custom errors.
  - Keep modules focused: `auth.rs`, `api.rs`, `playback.rs`, `ipc.rs`, `main.rs`.
- **QML (`BarWidget.qml`, `Service.qml`):**
  - Pure declarative QML with small inline helper functions.
  - No direct filesystem mutations or heavy processing in QML; delegate all business logic to `tidal-daemon`.
  - Always handle disconnected socket state gracefully with visual fallback.
