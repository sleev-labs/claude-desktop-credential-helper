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

## License

Apache-2.0
