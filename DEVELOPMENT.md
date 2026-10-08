# Development Requirements & Environment Setup

This document details the toolchains, system runtime packages, development workflow, and verification suites required to develop, build, test, and contribute to **Omarchy Tidal Music Player (`jaj.tidal`)**.

End-user installation is zero-dependency with respect to build tools: the tracked
Linux x86-64 `bin/tidal-daemon` works out of the box on a compatible Omarchy system,
without Rust, Cargo, or compilation. Existing system runtime libraries,
Omarchy/Quickshell, `mpv`, PipeWire, and `xdg-open` are still required.
The toolchain and audit utilities below are for development and verification.

---

## 1. Toolchain Manager: `mise` (mise-en-place)

The project standardizes on **[`mise`](https://mise.jdx.dev/)** (formerly `rtx`) as the recommended runtime and toolchain manager. A [`mise.toml`](file:///home/jaj/Projects/omarchy-tidal-music-player/mise.toml) configuration file is provided in the repository root to pin the exact development toolchain:

```toml
[tools]
rust = "stable"
```

### Primary Setup with `mise`

1. **Install `mise`** (if not already installed):
   - On Arch Linux / Omarchy:
     ```bash
     omarchy pkg add mise
     # or
     sudo pacman -S mise
     ```
   - Via the official standalone installer:
     ```bash
     curl https://mise.run | sh
     ```

2. **Activate and install project toolchains**:
   Inside the repository root:
   ```bash
   mise install
   ```
   Or explicitly set/use the pinned Rust version:
   ```bash
   mise use rust@stable
   ```

3. **Verify toolchain status**:
   ```bash
   mise current
   cargo --version
   rustc --version
   ```
   `mise` automatically exposes `cargo` and `rustc` in your shell environment whenever you navigate into the project directory.

---

### Alternative Toolchain Setups

If you prefer not to use `mise`, you can use either of the following supported methods:

#### Option A: Omarchy / Arch Linux Package Manager
Install the native system Rust package:
```bash
omarchy pkg add rust
# or
sudo pacman -S rust cargo
```

#### Option B: Official `rustup`
Install Rust via the official Rust toolchain installer:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
```

---

## 2. System & Runtime Dependencies

The Tidal plugin architecture separates the lightweight front-end UI (`quickshell`) from the headless backend daemon (`tidal-daemon`), coordinating with `mpv` for audio decoding and PipeWire for output routing.

### Required Packages

| Package | Purpose in Project | Command / Binary | Arch / Omarchy Package |
| :--- | :--- | :--- | :--- |
| **`mpv`** | Headless playback engine spawned and coordinated by `tidal-daemon` via JSON IPC. Decodes MPEG-DASH and unencrypted FLAC streams with bit-perfect PipeWire output. | `/usr/bin/mpv` | `omarchy pkg add mpv` |
| **`pipewire`** | Low-latency audio server and routing engine. `mpv` outputs directly to the PipeWire sink (`--ao=pipewire`). | `/usr/bin/pipewire` | `omarchy pkg add pipewire pipewire-pulse` |
| **`quickshell`** | Native Qt/QML desktop shell engine powering Omarchy. Renders [`BarWidget.qml`](file:///home/jaj/Projects/omarchy-tidal-music-player/BarWidget.qml) and runs background service [`Service.qml`](file:///home/jaj/Projects/omarchy-tidal-music-player/Service.qml). | `/usr/bin/quickshell` | Preinstalled with Omarchy (or `quickshell-git`) |
| **`xdg-utils`** | Provides `xdg-open` to automatically launch the default web browser for OAuth 2.0 Device Flow authorization (`https://link.tidal.com`). | `/usr/bin/xdg-open` | `omarchy pkg add xdg-utils` |
| **`curl`** | HTTP utility used by installation scripts, API diagnostics, and network checks. | `/usr/bin/curl` | `omarchy pkg add curl` |
| **`jq`** | Command-line JSON parser required by [`scripts/verify-omarchy-compliance.sh`](file:///home/jaj/Projects/omarchy-tidal-music-player/scripts/verify-omarchy-compliance.sh) to inspect [`manifest.json`](file:///home/jaj/Projects/omarchy-tidal-music-player/manifest.json). | `/usr/bin/jq` | `omarchy pkg add jq` |
| **`git`** | Version control and tree integrity auditing. | `/usr/bin/git` | `omarchy pkg add git` |

### One-Line Dependency Installation (Arch / Omarchy)

To install all system dependencies at once:
```bash
omarchy pkg add mpv pipewire pipewire-pulse xdg-utils curl jq git
# or using pacman directly:
sudo pacman -S --needed mpv pipewire pipewire-pulse xdg-utils curl jq git
```

### Optional & Recommended Tools

- **`binutils` (`strip`, `stat`, `du`, `file`)**: Required by [`scripts/verify-size.sh`](file:///home/jaj/Projects/omarchy-tidal-music-player/scripts/verify-size.sh) and [`scripts/build.sh`](file:///home/jaj/Projects/omarchy-tidal-music-player/scripts/build.sh) for binary size verification and debug symbol stripping (`sudo pacman -S binutils`).
- **`upx`**: Optional executable packer used for release compression to bring the daemon under 800 KB (`omarchy pkg add upx` or `sudo pacman -S upx`).
- **`playerctl`**: CLI controller for testing MPRIS 2.0 desktop integration and media keys (`omarchy pkg add playerctl`).

---

## 3. Development Workflow & Scripts

The repository provides automated scripts in the [`scripts/`](file:///home/jaj/Projects/omarchy-tidal-music-player/scripts) directory to streamline building, testing, and compliance verification.

### 1. Building the Daemon (`./scripts/build.sh`)
Compiles `tidal-daemon` in release mode with size optimization profiles:
```bash
./scripts/build.sh
```
- Applies Cargo profile: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`.
- Automatically strips debug symbols and `.comment` / `.note` sections using `strip`.
- Outputs binary to `backend/target/release/tidal-daemon`.
- Copies the stripped release artifact to tracked `bin/tidal-daemon` with mode
  `755`, then verifies the bundle is at most **1,800,000 bytes (1.8 MB)**.
- Commit the updated bundle with backend changes. `Service.qml` checks this
  executable first, falling back to the development release path if unavailable.

### 2. Running Unit Tests (`cargo test`)
Runs the comprehensive Rust unit test suite:
```bash
cargo test --manifest-path backend/Cargo.toml
# or
(cd backend && cargo test)
```
Covers:
- `auth::tests`: Device authorization JSON parsing, session token persistence, expiration checks.
- `playback::tests`: Base64 BTS playbackinfo manifest decoding and error handling.
- `ipc::tests`: JSON-RPC IPC message serialization and deserialization.

### 3. Running Verification Suites

#### Complete Gate Runner (`./scripts/verify.sh`)
Runs all checks required by the project's gate policy before completing milestones or submitting pull requests:
```bash
./scripts/verify.sh
```
Executes:
1. Omarchy Official Compliance Audit
2. Backend unit tests (`cargo test`)
3. Release binary size and debug symbol audit

#### Omarchy Compliance Audit (`./scripts/verify-omarchy-compliance.sh`)
Validates that the plugin complies 100% with Omarchy desktop shell standards:
```bash
./scripts/verify-omarchy-compliance.sh
```
Checks:
- `manifest.json` schema validation (schemaVersion = 1, required fields, entry point mappings).
- Plugin ID namespace compliance (non-reserved, regex pattern).
- Ban on internal symlinks (prevents path traversal escapes in shell plugins).
- Invocation of official `omarchy plugin validate` or `omarchy-plugin-validate` if installed.
- Dynamic theming compliance (all QML colors bound to `qs.Commons.Color`, zero hardcoded hex codes).
- Bundled `bin/tidal-daemon` must exist, be executable, be a stripped ELF binary,
  and be at most 1.8 MB; this requirement cannot be skipped.

#### Binary Size & Symbol Auditor (`./scripts/verify-size.sh`)
Enforces the binary size budget:
```bash
./scripts/verify-size.sh
```
- Mandatory bundled binary limit: **<= 1.8 MB**, exactly **1,800,000 bytes**;
  checks use bytes without rounding.
- Requires an executable, stripped ELF at `bin/tidal-daemon`, not merely absence
  of debug information (`file` check).
- If a development release artifact exists, also enforces its original
  **2.5 MiB** (`2560 * 1024` bytes) ceiling and byte-for-byte bundle agreement.
- Works on a fresh clone with no `backend/target` directory.

#### Project Status Dashboard (`./scripts/status.sh`)
Prints a quick terminal summary of milestone progress, completed tasks, and active goals:
```bash
./scripts/status.sh
```

---

## 4. Omarchy Theming & Design Rules

When modifying or adding QML code ([`BarWidget.qml`](file:///home/jaj/Projects/omarchy-tidal-music-player/BarWidget.qml), [`Service.qml`](file:///home/jaj/Projects/omarchy-tidal-music-player/Service.qml)):

1. **Dynamic Theming Binding**:
   - **Never** hardcode hex colors (e.g. `#1e1e2e` or `#ffffff`).
   - Use `qs.Commons.Color`:
     - `Color.foreground` / `Color.foregroundMuted`
     - `Color.background` / `Color.backgroundElevated`
     - `Color.accent`
     - `Color.urgent`
   - The UI must react dynamically when switching themes via `omarchy theme set <theme>` with zero shell restarts.

2. **Spacing & Typography**:
   - Use `qs.Commons.Style`:
     - `Style.space(Style.scaleX)` for paddings and margins.
     - `Style.cornerRadius` for corner rounding.
     - `Style.font.body` / `Style.font.subtext` for fonts.

3. **Symlink Prohibition**:
   - Do **not** create internal symlinks inside the repository or plugin directory, or symlink the installed plugin root. Omarchy rejects these for security reasons.

---

## 5. Local Shell Testing

To test the plugin live in your local Omarchy desktop environment:

1. **Build and install a standalone plugin into Omarchy**:
   ```bash
   ./scripts/build.sh
   ./install.sh
   ```
   This refreshes the tracked bundle and copies the source and `bin/tidal-daemon`
   to `~/.config/omarchy/plugins/jaj.tidal`. For an unchanged checkout, installation
   needs only `./install.sh`, with no build or Rust toolchain.
   Run `./install.sh` again after source edits or backend rebuilds.

2. **Enable plugin in status bar**:
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
   Or enable via the CLI:
   ```bash
   omarchy plugin enable jaj.tidal
   ```

3. **Verify the service and widget**:
   - The bar will display the Tidal music icon.
   - Clicking the icon opens the flyout pairing/player card.
   - Run `./scripts/verify.sh` to ensure all tests pass.
