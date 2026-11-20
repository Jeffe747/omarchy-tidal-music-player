# Release binary provenance

## v1.0.78 release artifacts

The checked-in executable `bin/tidal-daemon` is the official x86_64 artifact
produced by the v1.0.78 GitHub Actions [build run 44684738874](https://github.com/Jeffe747/omarchy-tidal-music-player/actions/runs/44684738874)
from source commit `4bc9f46e805ebe0b325e0b508c3ce1b1d8385f26`.
The tag workflow publishes these same binaries only if the committed
checksums, x86_64 bundle, and runtime pins match the built artifacts. The
x86_64 release artifact is 1,831,808 bytes and its digest matches
`SHA256SUMS` and the runtime pin in `Service.qml`.

The workflow uses Ubuntu 24.04 and rustc 1.99.1
(`58cf8f9c3`, 2026-10-13) to build from the Rust source in `backend/`
with Cargo for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`. The package uses
Rust edition 2021. Build locally with `cargo build --release --manifest-path
backend/Cargo.toml --locked`, then apply:

```sh
strip --strip-all --remove-section=.comment --remove-section='.note*' --remove-section=.eh_frame --remove-section=.eh_frame_hdr backend/target/release/tidal-daemon
```

For x86_64 the workflow sets `CFLAGS_x86_64_unknown_linux_gnu=-march=x86-64
-mtune=generic` and `RUSTFLAGS="-C target-cpu=x86-64 -C
link-arg=-Wl,-z,pack-relative-relocs"`. Packed relative relocations require
glibc 2.36 or newer. The release test runs ring's X25519 key exchange and
Ed25519 signing/verification with `qemu-x86_64 -cpu IvyBridge` (no BMI1/AVX2);
this emulates the crypto path implicated in the earlier SIGILL but is not a
substitute for testing the published executable on the affected laptop.

The release profile in `backend/Cargo.toml` sets `opt-level = "z"`, `lto =
true`, `codegen-units = 1`, `panic = "abort"`, and `strip = true`.

| Artifact | Size | SHA-256 |
| --- | ---: | --- |
| `bin/tidal-daemon` / `tidal-daemon-x86_64` | 1,831,808 bytes | `afc7fcc8c380325e1e6b97eed8ba95c6509cae7a6c64bfba72659441412970da` |
| `tidal-daemon-aarch64` | 2,036,752 bytes | `77ef1e90b8a49c3dcf7c5f8ed6f5d273af3aceebd6058c9d67380c31fb8d31de` |

The local Cargo build may differ byte-for-byte from the official Ubuntu-built
release artifact. `scripts/verify-size.sh` therefore accepts the bundled x86_64
artifact when its digest matches the canonical `SHA256SUMS` entry. `Service.qml`
checks both the bundled executable's and the cached state executable's SHA-256
against the architecture-specific pin before selecting either for execution. The
bootstrap verifies downloads against the same pins and uses release tag
`v1.0.78`.

## Release workflow

`.github/workflows/release.yml` builds x86_64 with Cargo and applies the strip
command above. It does not install or run UPX. The published artifact checksums
are recorded in the repository `SHA256SUMS` and pinned in `Service.qml`.
