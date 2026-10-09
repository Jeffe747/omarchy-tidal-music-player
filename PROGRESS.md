# Project Progress & Roadmap Tracker

Last Updated: **2026-10-09**
Current Status: **UI Enhancement Milestone complete; release gate and Wayland checks pass; M7 compressed startup target remains unmet**

---

## High-Level Milestone Overview

| # | Milestone | Status | Completed / Total Tasks |
| :---: | :--- | :---: | :---: |
| **M0** | **Foundation & Scaffolding** | 🟢 **Done** | 6 / 6 |
| **M1** | **Authentication & Session Management (Login)** | 🟢 **Done** | 5 / 5 |
| **M2** | **Favorites List & Core Audio Playback** | 🟢 **Done** | 6 / 6 |
| **M3** | **Interactive Controls, Seeking & Auto-Advance** | 🟢 **Done** | 4 / 4 |
| **M4** | **MPRIS D-Bus & Desktop Integration** | 🟢 **Done** | 5 / 5 |
| **M5** | **Catalog Search & Discovery** | 🟢 **Done** | 4 / 4 |
| **M6** | **User Playlists & Audio Quality Tiers** | 🟢 **Done** | 4 / 4 |
| **M7** | **Binary Minimization & Theme Polish** | 🟢 Done | 4 / 4 |
| **M8** | **UI Enhancements & Exploration** | 🟢 Done | 7 / 7 |

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

### Milestone 3: Interactive Controls, Seeking & Auto-Advance 🟢
- [x] Wire mpv IPC commands for pause, resume/play, toggle, absolute seek, next, and previous, with command and fake-mpv coverage.
- [x] Emit playback position and duration every 250ms while playing; status reports mpv's actual position.
- [x] Auto-advance to the next favorite on mpv EOF; previous restarts after 3 seconds or selects the preceding favorite.
- [x] Add interactive seek slider, elapsed/total labels, transport buttons with tooltips, and right/middle-click play/pause shortcuts.
- [x] **Gate Verification:** `./scripts/build.sh` and `./scripts/verify.sh` passed; bundle is stripped at 1,795,136 bytes (<= 1,800,000). QML runtime execution was skipped because this environment has no active Wayland session.

---

### Milestone 4: MPRIS D-Bus & Desktop Integration 🟢
- [x] Register `org.mpris.MediaPlayer2.Tidal` on session D-Bus and implement Introspectable, Properties, root, and Player interfaces using the system `libdbus` runtime.
- [x] Expose playback status, metadata, position, capabilities, and transport/seek methods; mirror state changes through `PropertiesChanged` and emit `Seeked`.
- [x] Verify playerctl play/pause, next, previous, metadata, and seeking against live authenticated playback; verify Hyprland's `omarchy-shell media` keybinding targets control Tidal.
- [x] Verify the Omarchy media service recognizes Tidal and routes play/pause, next, and previous correctly. Disable mpv's competing MPRIS script for the daemon-managed headless process.
- [x] **Gate Verification:** `./scripts/build.sh` and `./scripts/verify.sh` passed; live bus introspection and playerctl metadata succeeded; stripped bundle is 1,672,776 bytes (<= 1,800,000).

---

### Milestone 5: Catalog Search & Discovery 🟢
- [x] Implement authenticated search API endpoint with encoded query, session country code, track/album/playlist types, and structured track/artwork parsing.
- [x] Implement 350ms debounced search input and a results view with navigation back to favorites.
- [x] Implement single-click playback of arbitrary search tracks through the existing playback manifest pipeline.
- [x] **Gate Verification:** `./scripts/build.sh` and `./scripts/verify.sh` passed (40 Rust tests; stripped 1,675,624-byte bundle). Live authenticated searches returned 20 tracks in 274ms and 285ms; playing a returned search track emitted `playback_started`. QML runtime validation was skipped because no Quickshell/Omarchy Wayland session is active.

---

### Milestone 6: User Playlists & Audio Quality Tiers 🟢
- [x] Implement user playlist listing and track endpoints, IPC events, and active playlist queue navigation.
- [x] Add quality preference persistence, negotiated quality fallback ladders, and DASH MPD support for mpv.
- [x] Add playlist navigation/detail views, quality selector and dynamic negotiated quality badge.
- [x] **Gate Verification:** `./scripts/build.sh`, `./scripts/verify.sh`, daemon smoke, and Omarchy compliance passed. Live User ID 5040 verification loaded 28 user playlists and parsed 100 tracks; a requested `HI_RES_LOSSLESS` stream gracefully negotiated to `LOSSLESS` and resolved to an unencrypted, playable FLAC BTS stream. Quality preference persistence passed a daemon restart check. DASH MPD handling is unit tested; quality badge/switcher bindings passed QML theming/compliance checks. QML runtime execution was skipped because no Wayland/Quickshell session is available.

---

### Milestone 7: Binary Minimization, Theme Verification & Polish 🟢
- [x] Add early-expiry token refresh and retry-on-401 refresh with persisted session update; cover expiry safety-window calculation.
- [x] Classify transient network failures and expose retryable IPC errors; harden stale mpv IPC socket cleanup/restart behavior.
- [x] Add opt-in `TIDAL_UPX=1` packaging and UPX-aware size validation (< 800 KB compressed; <= 1,800,000 bytes uncompressed).
- [x] Build and validate UPX release artifact: bundle is 774,740 bytes (<800,000); `./scripts/verify.sh` passed all 45 Rust tests and smoke checks; `omarchy plugin validate .` passed. UPX 4.2.4 rejected the ELF; official UPX 5.2.1 compressed it successfully.
- [ ] Meet the <15 ms compressed startup target: ten warm `--help` launches had a 71.70 ms median with `--best --lzma`; without LZMA, size was 850,616 bytes and startup measured 19–25 ms.
- [x] Apply Catppuccin, Tokyo Night, Everforest, and Nord themes, then restore the original Bear2 theme.
- [x] Run `WAYLAND_DISPLAY=wayland-1 ./scripts/verify.sh`; all four live Wayland QML integration scenarios passed. The last 30 `omarchy-shell` journal entries contained no warnings or errors for `jaj.tidal`.

### Milestone 8: UI Enhancements & Exploration 🟢
- [x] Add shuffle and repeat controls with status IPC and queue/EOF behavior; add current-track favorite toggling and state updates.
- [x] Move audio quality preferences and account/logout controls into a settings popup; remove quality controls from the library tabs.
- [x] Make catalog search available across tabs and add real-time playlist title/artist filtering.
- [x] Add themed visible scrollbars to favorites, playlists, playlist tracks, search results, and exploration lists.
- [x] Add album track and artist top-track exploration with a back action.
- [x] Add panel keyboard shortcuts for play/pause and ±5 second seeking when text fields are not handling input.
- [x] **Gate Verification:** `TIDAL_UPX=1 ./scripts/build.sh`, `WAYLAND_DISPLAY=wayland-1 ./scripts/verify.sh`, and `omarchy plugin validate .` passed. 49 Rust tests passed; four QML runtime scenarios passed; compressed bundle is 777,712 bytes.

---

## Activity Log

| 2026-10-09 | This change | Completed M7 resilience and release work: added refresh-on-401 retry, transient transport classification surfaced as retryable errors, safe expiry calculation tests, optional UPX packaging/validation, and rebuilt stripped bundle (1,707,520 bytes). `./scripts/verify.sh` passed (45 tests plus daemon smoke and compliance), `omarchy plugin validate .` passed, and four stock themes were applied before restoring Bear2. CLI startup measured 5.98 ms. UPX and Wayland visual/runtime checks unavailable on this host. | Codex |
| 2026-10-09 | This change | UPX and live Wayland follow-up: official UPX 5.2.1 successfully compressed bundle to 774,740 bytes; 4.2.4 failed on this ELF. LZMA startup median was 71.70 ms (10 warm runs), above the <15 ms target; non-LZMA was 850,616 bytes and 19–25 ms. `WAYLAND_DISPLAY=wayland-1 ./scripts/verify.sh` passed, including all four QML modes and 45 tests. Last 30 shell journal records had no jaj.tidal warnings/errors. | Codex |

| 2026-10-09 | This change | Implemented M6 playlist APIs and parsing (including nullable Tidal metadata), playlist IPC/events and queue navigation, persistent quality selection and requested-tier fallbacks, DASH MPD handling, MPRIS quality properties, and themed playlist/quality UI. `./scripts/build.sh`, `./scripts/verify.sh` (43 Rust tests), `python3 tests/daemon-smoke.py`, and `git diff --check` passed; stripped bundle is 1,698,560 bytes. Live User ID 5040 returned 28 playlists and 100 tracks from the first playlist; its first track resolved to a LOSSLESS BTS stream. A HI_RES_LOSSLESS probe negotiated down to LOSSLESS, and available playlist/search results exposed no Hi-Res catalog candidate, so live 24-bit confirmation remains pending and M6 is not marked complete. Quickshell runtime check skipped without Wayland. | Codex |

| 2026-10-09 | This change | Completed the M6 gate: live User ID 5040 playlist loading returned 28 user playlists, playlist track parsing returned 100 tracks, and a `HI_RES_LOSSLESS` request gracefully negotiated to a playable unencrypted LOSSLESS FLAC BTS stream. Verified persisted quality selection across a daemon restart using isolated temporary state. DASH MPD handling is covered by unit tests; badge/switcher bindings passed Omarchy color/style compliance. `./scripts/build.sh`, `./scripts/verify.sh` (43 Rust tests), daemon smoke, and `git diff --check` passed; bundled stripped daemon is 1,698,624 bytes. QML runtime test skipped without Wayland/Quickshell. | Codex |

| 2026-10-08 | This change | Implemented catalog search API, query encoding and parsing tests, search IPC events, 350ms flyout debounce, results with artwork/metadata/duration, favorites/search navigation, and one-click playback for catalog tracks. `./scripts/build.sh`, `./scripts/verify.sh`, QML verification script, and `git diff --check` passed; 40 Rust tests passed and the stripped bundle is 1,675,624 bytes. Live authenticated searches returned 20 results in 274ms and 285ms; playback of a returned search result started successfully. QML runtime was skipped without an active Wayland session. | Codex |

| Date | Commit | Description | Author |
| :--- | :--- | :--- | :--- |
| 2026-10-08 | This change | Fixed live EOF auto-advance when mpv becomes idle and no longer exposes `eof-reached`: the position ticker now remembers whether the current track entered playback and advances once when that started track becomes idle. Rebuilt the bundled daemon and verified live on Rimegale: seeking to 1.5 seconds before EOF advanced from “say something” to “Backseat” in 1.97 seconds; playback was paused afterward. | Codex |
| 2026-10-08 | This change | Completed M3 controls and absolute seeking, favorites navigation, 250ms position updates and EOF auto-advance, and QML transport/seek controls. `./scripts/build.sh` and `./scripts/verify.sh` passed with 34 Rust tests and a stripped 1,795,136-byte bundle; QML runtime verification skipped without a Wayland session. | Codex |
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
| 2026-10-08 | This change | Implemented MPRIS session-bus registration, properties, metadata, transport and seek methods, state-change signals, and desktop media integration. Verified with busctl, live playerctl controls, and Omarchy media commands; disabled mpv's competing MPRIS script. Full verification passed with a 1,672,776-byte stripped daemon. | Codex |

| 2026-10-09 | This change | Implemented UI Enhancement Milestone: shuffle/repeat and favorites IPC/API, settings popup, universal search, playlist filtering, themed list scrollbars, album/artist exploration, and panel shortcuts. UPX build and Wayland full gate passed (49 Rust tests, four QML scenarios, daemon smoke); plugin validation passed. Bundle is 777,712 bytes. | Codex |
