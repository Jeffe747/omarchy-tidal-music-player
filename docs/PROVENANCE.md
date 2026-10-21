# Release binary provenance

## v1.0.68 release artifacts

The checked-in executable `bin/tidal-daemon` is the official x86_64 artifact
published by the v1.0.68 GitHub Actions release workflow. The release was
created by [workflow run 39658587526](https://github.com/Jeffe747/omarchy-tidal-music-player/actions/runs/39658587526)
and published at [v1.0.68](https://github.com/Jeffe747/omarchy-tidal-music-player/releases/tag/v1.0.68).
The x86_64 release artifact is 1,831,176 bytes and its digest matches
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
| `bin/tidal-daemon` / `tidal-daemon-x86_64` | 1,831,176 bytes | `ee3ae9fd6fd4cbd5a3c2a97f9bc9c48122b0588c19f2337862807c01f19d0987` |
| `tidal-daemon-aarch64` | Published by release workflow | `fd87b08314de26cf7b8c023237a5bf37dc4dae36d75e38046001346b2ce87c6c` |

The local Cargo build may differ byte-for-byte from the official Ubuntu-built
release artifact. `scripts/verify-size.sh` therefore accepts the bundled x86_64
artifact when its digest matches the canonical `SHA256SUMS` entry. `Service.qml`
checks the bundled executable's SHA-256 against the architecture-specific pin
before selecting it for execution. The stable bootstrap verifies downloads
against the same pins and uses release tag `v1.0.68`.

## Release workflow

`.github/workflows/release.yml` builds x86_64 with Cargo and applies the strip
command above. It does not install or run UPX. The development release workflow
uses the same x86_64 strip command. The published artifact checksums are recorded
in the repository `SHA256SUMS` and pinned in `Service.qml`.
