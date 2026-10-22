# Release binary provenance

## v1.0.73 release artifacts

The checked-in executable `bin/tidal-daemon` is the official x86_64 artifact
produced by the v1.0.73 GitHub Actions [build run 39742315300](https://github.com/Jeffe747/omarchy-tidal-music-player/actions/runs/39742315300).
The tag workflow publishes these same binaries only if the committed
checksums, x86_64 bundle, and runtime pins match the built artifacts. The
x86_64 release artifact is 1,821,704 bytes and its digest matches
`SHA256SUMS` and the runtime pin in `Service.qml`.

The workflow builds from the Rust source in `backend/` with Cargo for
`x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`. The package uses
Rust edition 2021. Build locally with `cargo build --release --manifest-path
backend/Cargo.toml --locked`, then apply:

```sh
strip --strip-all --remove-section=.comment --remove-section='.note*' --remove-section=.eh_frame --remove-section=.eh_frame_hdr backend/target/release/tidal-daemon
```

The release profile in `backend/Cargo.toml` sets `opt-level = "z"`, `lto =
true`, `codegen-units = 1`, `panic = "abort"`, and `strip = true`.

| Artifact | Size | SHA-256 |
| --- | ---: | --- |
| `bin/tidal-daemon` / `tidal-daemon-x86_64` | 1,821,704 bytes | `ef34840b030d22d097340ac8873bb165a6f21a4fa8fabe3c4429eb78990e3ca9` |
| `tidal-daemon-aarch64` | 2,036,752 bytes | `82c5ef041249449f7d2e06e4a83e63ddff99528aa73909d44b2ad716613ac84c` |

The local Cargo build may differ byte-for-byte from the official Ubuntu-built
release artifact. `scripts/verify-size.sh` therefore accepts the bundled x86_64
artifact when its digest matches the canonical `SHA256SUMS` entry. `Service.qml`
checks both the bundled executable's and the cached state executable's SHA-256
against the architecture-specific pin before selecting either for execution. The
bootstrap verifies downloads against the same pins and uses release tag
`v1.0.73`.

## Release workflow

`.github/workflows/release.yml` builds x86_64 with Cargo and applies the strip
command above. It does not install or run UPX. The published artifact checksums
are recorded in the repository `SHA256SUMS` and pinned in `Service.qml`.
