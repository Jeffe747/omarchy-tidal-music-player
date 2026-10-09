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
   - The development release binary retains its **2.5 MiB** ceiling (`2560 * 1024` bytes). The tracked **`bin/tidal-daemon`** is mandatory, executable, stripped ELF, and must be **<= 1.8 MB (1,800,000 bytes)**.
   - `./scripts/build.sh` must refresh the bundle from `backend/target/release/tidal-daemon` with executable permissions. Commit the refreshed bundle alongside backend changes; keep `.gitignore` compatible with tracking it.
   - Installation must work out of the box without Rust, Cargo, or compilation on compatible Linux x86-64 Omarchy systems. Existing system runtime requirements still apply. `Service.qml` must prefer the executable bundle and retain the development-path fallback.
   - Do **NOT** pull in bloated dependencies (e.g., full `reqwest` with OpenSSL, massive web runtimes, or bundled FFmpeg libraries).
   - Use `ureq`, `serde`, and coordinate with system `mpv` for audio decoding.
4. **Omarchy Theming & Style Compliance**:
   - **NEVER** hardcode hex colors or arbitrary margins in QML.
   - Always bind to `qs.Commons.Color` (`Color.foreground`, `Color.background`, `Color.accent`, `Color.muted`, `Color.urgent`).
   - Always use `qs.Commons.Style` (`Style.space()`, `Style.cornerRadius`, `Style.font.*`).
   - The UI must dynamically react to `omarchy theme set <theme>` with zero restarts.
5. **Manifest Integrity**:
   - Any change to `manifest.json` must be verified with `omarchy plugin validate .`.

---

## 3. Standard Commands & Verification

Every agent must use these standard project scripts:

| Command | Action |
| :--- | :--- |
| `./scripts/status.sh` | Prints live milestone status, completed tasks, and active goals. |
| `./scripts/build.sh` | Compiles and strips the release daemon, refreshes executable `bin/tidal-daemon`, and audits the bundle. |
| `(cd backend && cargo test)` | Runs all unit tests. |
| `./scripts/verify-omarchy-compliance.sh` | Audits 100% of official Omarchy security, schema, symlink, and theming rules. |
| `./scripts/verify-size.sh` | Requires executable, stripped `bin/tidal-daemon` <= 1,800,000 bytes; checks any development release and bundle agreement. |
| `./scripts/verify.sh` | **Full gate runner:** runs Omarchy compliance audit, unit tests, and binary size audit. |
| `./install.sh` | Verifies the bundle, copies a standalone plugin without build caches or compilation, validates it, and triggers shell rescan. |

---

## 4. Milestone Progression & Current Status

The live granular task checklist and activity log are maintained in **[`PROGRESS.md`](PROGRESS.md)**.
Implementation is strictly phased into 7 milestones:

| # | Milestone | Status | Gate Pre-requisite to Advance |
| :---: | :--- | :---: | :--- |
| **M1** | **Authentication & Session Management (Login)** | 🟢 **Done** | Unit tests pass for `auth.rs`; device code generated; tokens persisted to `~/.local/state/omarchy/tidal/session.json`; UI shows "Connected". |
| **M2** | **Favorites List & Core Audio Playback** | 🟢 **Done** | Live Rimegale verification: authenticated favorites, LOSSLESS BTS stream, headless mpv playback, PipeWire output, and IPC status. |
| **M3** | **Interactive Controls, Seeking & Auto-Advance** | 🟢 **Done** | Controls and queue behavior implemented and covered by mpv command/fake-mpv tests; QML runtime verification remains unavailable without Wayland. |
| **M4** | **MPRIS D-Bus & Desktop Integration** | 🟢 **Done** | Live D-Bus introspection, playerctl transport/metadata/seeking, Hyprland media keybinding, and Omarchy media service verified. |
| **M5** | **Catalog Search & Discovery** | 🟢 **Done** | Live searches returned results within 274–285 ms; selected search track started playback. |
| **M6** | **User Playlists & Audio Quality Tiers** | 🟢 **Done** | Live playlists/tracks loaded; HI_RES_LOSSLESS request gracefully negotiated to playable LOSSLESS; preference persistence verified. |
| **M7** | **Binary Minimization & Theme Polish** | 🟢 **Release verified** | Full test suite, plugin validation, compressed size (< 800 KB), and four Wayland QML scenarios pass. UPX startup target < 15 ms remains unmet (71.70 ms median with LZMA); see `PROGRESS.md`. |

---

## 5. Milestone Verification Gates

Before an agent claims a milestone as complete, it **must** run the corresponding verification checklist:

### Gate M1: Login Verification
```bash
./scripts/verify.sh
```
- [x] `auth::tests::test_device_auth_info_deserialization` passes.
- [x] `auth::tests::test_session_serialization_roundtrip` passes.
- [x] Running daemon or test harness receives user code from Tidal.
- [x] Confirming on `link.tidal.com` persists session to `~/.local/state/omarchy/tidal/session.json`.
- [x] Bar flyout updates from pairing prompt to "Connected" without restarting the shell.
- [x] Binary size is < 2.5 MB.

### Gate M2: Favorites & Playback Verification
```bash
./scripts/verify.sh
```
- [x] `playback::tests::test_parse_bts_manifest` passes.
- [x] Unit tests for favorites JSON parsing pass.
- [x] Daemon resolves a playable stream from the live `playbackinfopostpaywall` endpoint (LOSSLESS BTS).
- [x] Audio stream plays through PipeWire; active output streams confirmed on ALC233 Analog.
- [x] Album artwork and track metadata render on `BarWidget.qml` (QML runtime fixtures verified).

### Gate M3: Controls & Seeking Verification
- [x] Seeking and transport IPC commands are covered by command/fake-mpv tests; live MPRIS seek and transport were verified with `playerctl`.
- [x] Queue next/previous and EOF auto-advance behavior are covered by playback tests.
- [x] Transport buttons and bar shortcuts are implemented in `BarWidget.qml`.
- [ ] QML runtime interaction verification (including slider latency and right-click) remains pending because no Wayland/Quickshell session was available.

### Gate M4: MPRIS Verification
- [x] Live D-Bus introspection reports valid MPRIS properties.
- [x] `playerctl` transport, seeking, status, and metadata work against live playback.
- [x] Hyprland's Omarchy media keybinding and Omarchy media service route controls to Tidal.

### Gate M5: Search Verification
- [x] Search input uses a 350ms debounce; live authenticated searches returned 20 tracks in 274ms and 285ms.
- [x] Playing a live search result started playback successfully.

### Gate M6: Playlists & Audio Quality Verification
- [x] Live account playlist listing and track loading succeeded (28 playlists; 100 tracks parsed).
- [ ] 24-bit Hi-Res playback remains unverified; the live `HI_RES_LOSSLESS` request negotiated to LOSSLESS.
- [x] Quality selector and negotiated quality badge bindings passed Omarchy color/style compliance checks.

### Gate M7: Final Polish & Release
- [x] Apply `catppuccin`, `tokyo-night`, `everforest`, and `nord`; original theme restored. All four QML runtime fixture scenarios passed with Wayland available.
- [x] Run `./scripts/build.sh`, `./scripts/verify-size.sh`, and `./scripts/verify.sh`.
- [x] Confirm UPX 5.2.1 compression yields 774,740 bytes and `verify-size.sh` passes. UPX 4.2.4 rejected this ELF.
- [ ] Confirm compressed startup below 15 ms. Measured 71.70 ms median for ten warm `--help` launches using `--best --lzma`; see `PROGRESS.md`.

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


## 7. UI Enhancement Milestone (M8)

The flyout includes shuffle and repeat transport state, current-track favorite mutation, an account/settings popup with audio quality selection and logout, universal catalog search, playlist track filtering, and album/artist exploration views. Favorites, playlists, playlist tracks, search results, and exploration lists use themed visible scrollbars. Panel shortcuts toggle playback with Space and seek five seconds with the arrow keys when a text input is not focused.

Backend IPC status exposes `shuffle`, `repeat_mode`, and `is_favorite`; catalog exploration commands load album tracks and artist top tracks. Verify this milestone with `TIDAL_UPX=1 ./scripts/build.sh`, `WAYLAND_DISPLAY=wayland-1 ./scripts/verify.sh`, and `omarchy plugin validate .`.
