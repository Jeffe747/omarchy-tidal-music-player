# Release binary provenance

## v1.0.62 x86_64 bundle

The checked-in executable `bin/tidal-daemon` is built from the Rust source in
`backend/` for the `x86_64-unknown-linux-gnu` target. The package uses Rust
edition 2021. Build with `cargo build --release --manifest-path
backend/Cargo.toml --locked`, then apply:

```sh
strip --strip-all --remove-section=.comment --remove-section='.note*' --remove-section=.eh_frame --remove-section=.eh_frame_hdr backend/target/release/tidal-daemon
```

The release profile in `backend/Cargo.toml` sets `opt-level = "z"`, `lto =
true`, `codegen-units = 1`, `panic = "abort"`, and `strip = true`.

| Artifact | Size | SHA-256 |
| --- | ---: | --- |
| `backend/target/release/tidal-daemon` | 1,730,256 bytes | `6c84f7b144b5e151483d6faadcf3917043aec7f94eb5bfd3eb718643e56b1f4a` |
| `bin/tidal-daemon` | 1,730,256 bytes | `6c84f7b144b5e151483d6faadcf3917043aec7f94eb5bfd3eb718643e56b1f4a` |

`./scripts/build.sh` confirmed the two x86_64 files are byte-for-byte identical.
`Service.qml` checks the bundled executable's SHA-256 against the pinned digest
before selecting it for execution. The stable bootstrap uses the same pin and
downloads release tag `v1.0.62`.

## Release workflow

`.github/workflows/release.yml` builds x86_64 with Cargo and applies the strip
command above. It does not install or run UPX. The development release workflow
uses the same x86_64 strip command.

## aarch64 limitation

This environment did not have `cross` or an aarch64 Rust target/toolchain, so a
v1.0.62 aarch64 artifact could not be built and independently hashed here. The
currently configured aarch64 digest remains the prior known release digest
`151eece893151952e94bab953d9ea0bfaa18d8d717c400c382210011982911cf`; it is
retained in `Service.qml` and `SHA256SUMS` for consistency. The aarch64 release
workflow will generate and publish the checksum for its newly built artifact.
The CI artifact digest must be recorded here and pinned in `Service.qml` and
the root `SHA256SUMS` once that build is available.
