# claude-desktop-cred

Credential helper for Claude Desktop's third-party inference (gateway) mode. Prints the OAuth token stored by the `claude` CLI in the `inferenceCredentialHelper` contract format — `{"token": "...", "headers": {}}` — refreshing and persisting it when expired.

## Requirements

A Claude subscription login via the `claude` CLI. `curl` on PATH (and `security` on macOS).

## Build

```sh
cargo build --release   # target/release/claude-desktop-cred
```

## Use

Point Claude Desktop's `inferenceCredentialHelper` at the binary and set `inferenceCredentialHelperTtlSec` to 300. Routing headers belong in Desktop's own `inferenceCustomHeaders`.

`claude-desktop-cred --help` / `--version` for the CLI surface; Desktop runs it with no arguments.

## Release

Binaries for linux-x64/arm64 (musl), darwin-x64/arm64, and win32-x64/arm64 are published to the sleev updates bucket under `claude-desktop-cred/<version>/` (immutable archives plus `manifest.json`) with a `claude-desktop-cred/<channel>.json` pointer. The `sleev` CLI installs the helper from there.

To release: bump `version` in `Cargo.toml` (and `Cargo.lock` via `cargo build`), merge to `main`, then tag that commit `v<version>` and push the tag. The tag must match `Cargo.toml` or the run fails before building. A prerelease version (`1.2.0-rc1`) publishes to the `beta` channel; anything else to `stable`. Every archive carries a GitHub build-provenance attestation: `gh attestation verify <archive> --repo sleev-labs/claude-desktop-credential-helper`.

The workflow can also be run manually (`workflow_dispatch`) as a dry run that lints, tests, and builds without publishing. Publishing needs the repository variables `SLEEVE_UPDATES_BUCKET`, `GCP_WORKLOAD_IDENTITY_PROVIDER`, and `GCP_SERVICE_ACCOUNT` (Workload Identity Federation; no stored keys).

## License

Apache-2.0
