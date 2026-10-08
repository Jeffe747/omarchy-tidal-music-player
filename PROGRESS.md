# Project Progress & Roadmap Tracker

Last Updated: **2026-10-08**  
Current Status: **Milestone 2 complete; live authenticated playback and PipeWire gate passed on Rimegale**

---

## High-Level Milestone Overview

| # | Milestone | Status | Completed / Total Tasks |
| :---: | :--- | :---: | :---: |
| **M0** | **Foundation & Scaffolding** | 🟢 **Done** | 6 / 6 |
| **M1** | **Authentication & Session Management (Login)** | 🟢 **Done** | 5 / 5 |
| **M2** | **Favorites List & Core Audio Playback** | 🟢 **Done** | 6 / 6 |
| **M3** | **Interactive Controls, Seeking & Auto-Advance** | ⚪ Queued | 0 / 4 |
| **M4** | **MPRIS D-Bus & Desktop Integration** | ⚪ Queued | 0 / 4 |
| **M5** | **Catalog Search & Discovery** | ⚪ Queued | 0 / 3 |
| **M6** | **User Playlists & Audio Quality Tiers** | ⚪ Queued | 0 / 4 |
| **M7** | **Binary Minimization & Theme Polish** | ⚪ Queued | 0 / 4 |

---

## Detailed Task Breakdown

### Milestone 0: Foundation, Scaffolding & Agent Gates 🟢
- [x] Create Omarchy plugin manifest ([`manifest.json`](manifest.json)) matching schema version 1.
- [x] Scaffold UI components ([`BarWidget.qml`](BarWidget.qml), [`Service.qml`](Service.qml)) using `qs.Commons` and `qs.Ui`.
- [x] Scaffold Rust backend crate ([`backend/Cargo.toml`](backend/Cargo.toml)) with size optimization profile (`opt-level = "z"`, `lto = true`, `strip = true`).
- [x] Build automated test runner ([`scripts/verify.sh`](scripts/verify.sh)) and size auditor ([`scripts/verify-size.sh`](scripts/verify-size.sh)).
- [x] Implement official Omarchy compliance validator ([`scripts/verify-omarchy-compliance.sh`](scripts/verify-omarchy-compliance.sh)).
- [x] Define agent contract and operating gate ([`AGENTS.md`](AGENTS.md)) and architecture plan ([`PLAN.md`](PLAN.md)).

---

### Milestone 1: Authentication & Session Management (Login) 🟢
- [x] Scaffold Device Authorization flow models in `auth.rs`.
- [x] Add unit tests for `DeviceAuthInfo` JSON deserialization and `Session` serialization roundtrip.
- [x] Implement end-to-end device code request (`POST /v1/oauth2/device_authorization`).
- [x] Implement token polling background loop with RFC 8628 handling (`slow_down`, `authorization_pending`, `expired_token`).
- [x] Connect IPC flow: widget opens `link.tidal.com` via `xdg-open`, polls daemon, and updates UI to "Connected".
- [x] **Gate Verification:** Pass Gate M1 (session tokens saved to `~/.local/state/omarchy/tidal/session.json`, binary size < 2.5 MB).

---

### Milestone 2: Favorites List & Core Audio Playback 🟢
- [x] Implement Tidal API client endpoint for fetching user favorite tracks.
- [x] Implement `playbackinfopostpaywall` manifest resolver (`application/vnd.tidal.bts` unencrypted FLAC), with legacy endpoint and quality-rejection fallbacks.
- [x] Launch headless `mpv` background runner with PipeWire audio sink (`--ao=pipewire`).
- [x] Connect playback IPC commands (`play_track`, `favorites_loaded`).
- [x] Render scrollable favorites list and now-playing artwork card in `BarWidget.qml`.
- [x] **Gate Verification:** Pass Gate M2 on Rimegale: authenticated session (User ID 5040), Tidal favorites fetched, unencrypted LOSSLESS BTS stream resolved, loaded into headless mpv, PipeWire output confirmed on ALC233 Analog (`output_FR`/`output_FL`), and player status verified over IPC.

Local verification passed: 28 Rust tests, four QML runtime scenarios (including
favorites loading/empty/error states, click dispatch, and now-playing artwork),
bundled-daemon IPC smoke, Omarchy compliance, and the binary size audit. Real mpv
startup arguments, stale-socket recovery, and child reaping were verified.
The stripped bundle is 1,786,456 bytes, using stable ELF relative-relocation
packing and no UPX. Live verification on Rimegale confirmed the authenticated
session (User ID 5040), favorites API response, unencrypted LOSSLESS BTS stream
resolution, headless mpv playback, active PipeWire output streams
(`output_FR`/`output_FL`) on ALC233 Analog, and player status over IPC. Gate M2
passed; M3 remains queued.

---

### Milestone 3: Interactive Controls, Seeking & Auto-Advance ⚪
- [ ] Wire mpv IPC commands for `pause`, `resume`, `toggle_pause`, and `seek`.
- [ ] Implement real-time position emitter (250ms interval) for seek bar synchronization.
- [ ] Implement queue manager auto-advancing to next favorite on track EOF.
- [ ] Add right-click toggle shortcut on the bar widget button.
- [ ] **Gate Verification:** Pass Gate M3 (seek latency < 100ms, auto-advance on EOF).

---

### Milestone 4: MPRIS D-Bus & Desktop Integration ⚪
- [ ] Register `org.mpris.MediaPlayer2.Tidal` on session D-Bus via `zbus`.
- [ ] Implement `org.mpris.MediaPlayer2.Player` interface (PlaybackStatus, Metadata, PlayPause, Seek).
- [ ] Verify global hardware media keys control Tidal across Hyprland.
- [ ] Verify Omarchy `omarchy.media` bar widget automatically recognizes Tidal player.
- [ ] **Gate Verification:** Pass Gate M4 (`playerctl` integration and media keys verified).

---

### Milestone 5: Catalog Search & Discovery ⚪
- [ ] Implement search API endpoint (`/v1/search?query=...&types=TRACKS,ALBUMS`).
- [ ] Implement debounced search input field (`TextField`) in flyout panel.
- [ ] Implement single-click playback from search results.
- [ ] **Gate Verification:** Pass Gate M5 (search query latency < 500ms, instant playback).

---

### Milestone 6: User Playlists & Audio Quality Tiers ⚪
- [ ] Implement user playlists fetch endpoints.
- [ ] Add support for MPEG-DASH Hi-Res FLAC (up to 24-bit / 192 kHz) streams.
- [ ] Add Quality Badge indicator in header and quality switcher dropdown in settings.
- [ ] **Gate Verification:** Pass Gate M6 (custom playlists load, 24-bit stream verified).

---

### Milestone 7: Binary Minimization, Theme Verification & Polish ⚪
- [ ] Full theme verification across 3+ Omarchy stock themes (`catppuccin`, `tokyo-night`, `everforest`).
- [ ] Run release build, symbol stripping, and optional UPX compression (< 800 KB).
- [ ] Test graceful recovery from network dropouts and token expiration.
- [ ] **Gate Verification:** Pass Gate M7 (100% tests pass, binary < 1.5 MB, theme seamless).

---

## Activity Log

| Date | Commit | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-10-08 | This change | Completed Gate M2 with live verification on Rimegale: authenticated session (User ID 5040), favorites fetched, unencrypted LOSSLESS BTS manifest resolved, stream loaded into headless mpv, active PipeWire output verified on ALC233 Analog (`output_FR`/`output_FL`), and player status confirmed over IPC. | Codex |
| 2026-10-08 | This change | Added the public default client token fallback for `4N3n6Q1x95LL5K7p`, making the bundled plugin usable without user-provided environment variables; custom client secrets remain environment-configurable. Updated auth coverage, daemon smoke expectations, and setup documentation. Retained the `--probe <track_id>` CLI and isolated DASH manifest filenames. `./scripts/build.sh` and `./scripts/verify.sh` passed: 33 Rust tests, bundled daemon smoke, Omarchy compliance, and stripped 1,790,704-byte bundle. QML runtime checks were skipped because this environment has no Quickshell/Omarchy Wayland session. M2 live authenticated playback and PipeWire verification remains pending on the remote laptop; no milestone advanced. | Codex |
| 2026-10-08 | This change | Fixed playback subStatus 4005 ("Asset is not ready for playback") with both stereo-only immersive-audio query flags, catalog-quality preference, and a bounded preferred/HIGH/LOW ladder. Retry legacy playbackinfo for 401/404 or quality-related errors; negotiate lower quality for playback 401/403/404, while stopping on transport, parsing, rate-limit, and server failures. Added track/attempt/retry diagnostics and fallback regression coverage. Release build and full verification gate passed: 32 Rust tests, four QML scenarios, isolated daemon smoke, compliance, and stripped 1,787,704-byte bundle. M2 live authenticated playback/PipeWire verification remains pending on the remote laptop; no milestone advanced. | Copilot |
| 2026-10-08 | This change | Fixed playback HTTP 404 by using `playbackinfopostpaywall`, retrying legacy `playbackinfo` once on 404, and retrying `HIGH` only for quality-related 401/403/404 errors. Resolved and cached the real country from `/v1/sessions`, replacing the hardcoded US fallback. Added redacted HTTP status/`userMessage`/`subStatus` diagnostics and private persistent `daemon.log` (0700 directory, 0600 file, startup truncation above 256 KiB), including IPC/API/MIME/mpv events. `scripts/build.sh` and `WAYLAND_DISPLAY=wayland-1 scripts/verify.sh` passed: 28 Rust tests, four QML scenarios, isolated daemon/log smoke, compliance, and stripped 1,786,456-byte bundle. M2 live playback gate remains pending on the remote laptop; no milestone advanced. | Copilot |
| 2026-10-08 | This change | Implemented M2 favorites pagination/parsing and artwork, session-country fallback, lossless BTS resolution with encryption rejection, shared playback state/IPC events, robust headless mpv lifecycle, and favorites/now-playing UI. Full `scripts/verify.sh` passed: 19 Rust tests, four QML runtime scenarios, isolated daemon smoke, compliance, and stripped 1,766,752-byte bundle. Live Tidal/PipeWire/`pw-cli` audio verification remains pending on the remote laptop; no milestone advanced. | Copilot |
| 2026-10-08 | This change | Bundled tracked, executable, stripped `bin/tidal-daemon` (1,792,536 bytes; limit 1,800,000) for out-of-the-box installation without Rust/Cargo or compilation. Builds refresh the bundle; the service prefers it with a development fallback; installation and verification enforce the bundle contract. Full `scripts/verify.sh` gate passed, including four daemon-resolution QML scenarios and all nine Rust tests; exact size-boundary and invalid-bundle checks passed. System runtime requirements remain unchanged; no milestone advanced. | Copilot |
| 2026-10-08 | `e2b8d35` | Initial project scaffolding, `PLAN.md`, QML UI components, and Rust daemon skeleton. | jaj |
| 2026-10-08 | `173cc4e` | Added 7-milestone progression plan, verification gates, unit tests, and size audit script. | jaj |
| 2026-10-08 | `d70a175` | Added `AGENTS.md` operational contract and agent gate. | jaj |
| 2026-10-08 | `f8a8aef` | Added official Omarchy compliance validator (`scripts/verify-omarchy-compliance.sh`). | jaj |
| 2026-10-08 | `4aaa8fa` | Implemented Milestone 1: OAuth 2.0 Device Flow, token polling, session persistence, and UI integration. | jaj |
| 2026-10-08 | This change | Investigated cached bar-widget loading failure; qualified panel controls, removed duplicate service, and fixed failed-socket recovery and deferred commands. Added QML runtime verification. Live shell restart remains blocked by the locked desktop session; no milestone advanced. | Copilot |
| 2026-10-08 | This change | Installed-path validation rejected the symlinked plugin root. Replaced symlink deployment with a validated standalone copy and made installer validation errors fatal. | Copilot |
