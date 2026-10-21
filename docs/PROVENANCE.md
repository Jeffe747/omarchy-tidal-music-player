# Release binary provenance

## v1.0.67 release artifacts

The checked-in executable `bin/tidal-daemon` is the official x86_64 artifact
published by the v1.0.67 GitHub Actions release workflow. The release was
created by [workflow run 39470998393](https://github.com/Jeffe747/omarchy-tidal-music-player/actions/runs/39470998393)
and published at [v1.0.67](https://github.com/Jeffe747/omarchy-tidal-music-player/releases/tag/v1.0.67).
The x86_64 release artifact is 1,830,152 bytes and its digest matches
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
| `bin/tidal-daemon` / `tidal-daemon-x86_64` | 1,830,568 bytes | `fb650d3ec85aa4b420ffe090eefd8e70fe26b8c199772298a0a0ced465b2b20c` |
| `tidal-daemon-aarch64` | Published by release workflow | `abd7bb9c9555248a64b8a818d20686ec4dedc10a029b6e6d22bc1e4d6173185a` |

The local Cargo build may differ byte-for-byte from the official Ubuntu-built
release artifact. `scripts/verify-size.sh` therefore accepts the bundled x86_64
artifact when its digest matches the canonical `SHA256SUMS` entry. `Service.qml`
checks the bundled executable's SHA-256 against the architecture-specific pin
before selecting it for execution. The stable bootstrap verifies downloads
against the same pins and uses release tag `v1.0.67`.

## Release workflow

`.github/workflows/release.yml` builds x86_64 with Cargo and applies the strip
command above. It does not install or run UPX. The development release workflow
uses the same x86_64 strip command. The published artifact checksums are recorded
in the repository `SHA256SUMS` and pinned in `Service.qml`.
