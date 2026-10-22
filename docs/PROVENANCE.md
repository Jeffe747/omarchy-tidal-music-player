# Release binary provenance

## v1.0.72 release artifacts

The checked-in executable `bin/tidal-daemon` is the official x86_64 artifact
produced by the v1.0.72 GitHub Actions [build run 39740597479](https://github.com/Jeffe747/omarchy-tidal-music-player/actions/runs/39740597479).
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
| `bin/tidal-daemon` / `tidal-daemon-x86_64` | 1,821,704 bytes | `cddfe4f641a9cdf6ff5bbc650fbf36d47d07174f97cce09816011833c2018b34` |
| `tidal-daemon-aarch64` | 2,036,752 bytes | `11f064401c7fed48f9ee6fedc20d584e46c1d985a9965fe2265df885b7ddb4ab` |

The local Cargo build may differ byte-for-byte from the official Ubuntu-built
release artifact. `scripts/verify-size.sh` therefore accepts the bundled x86_64
artifact when its digest matches the canonical `SHA256SUMS` entry. `Service.qml`
checks both the bundled executable's and the cached state executable's SHA-256
against the architecture-specific pin before selecting either for execution. The
bootstrap verifies downloads against the same pins and uses release tag
`v1.0.72`.

## Release workflow

`.github/workflows/release.yml` builds x86_64 with Cargo and applies the strip
command above. It does not install or run UPX. The published artifact checksums
are recorded in the repository `SHA256SUMS` and pinned in `Service.qml`.
