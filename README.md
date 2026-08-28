# claude-desktop-credential-helper

`claude-desktop-cred` prints the OAuth token stored locally by the `claude`
CLI in the format Claude Desktop expects from an
[`inferenceCredentialHelper`](https://claude.com/docs/third-party/claude-desktop/credential-helper),
so Desktop's [third-party inference (gateway) mode](https://claude.com/docs/third-party/claude-desktop/gateway)
can run on a Claude subscription instead of a static API key.

Claude Desktop ignores `ANTHROPIC_BASE_URL` and `settings.json`, and its
gateway mode has no claude.ai sign-in — a credential helper is the only
supported way to hand it a rotating subscription token.

## How it works

1. Reads the credential store the `claude` CLI writes: the macOS Keychain
   (service `Claude Code-credentials`), or `~/.claude/.credentials.json` on
   Linux and Windows (honouring `CLAUDE_CONFIG_DIR`).
2. If the access token is still valid, prints it.
3. If it has expired, redeems the stored refresh token at Anthropic's OAuth
   token endpoint, saves the new pair back to the same store, and prints the
   new access token.
4. Prints `{"token": "<access token>", "headers": {}}` on stdout and exits 0.
   Any failure exits non-zero with stdout untouched; guidance goes to stderr
   only when a user is present (`CLAUDE_HELPER_CONTEXT`).

The token is only ever written to stdout, for Claude Desktop to send to the
gateway you configured. This is not a bridge that exposes a subscription as an
API endpoint.

## Usage

Log in once with the `claude` CLI using a Claude subscription account, then
point Claude Desktop at the helper (Developer → Configure Third-Party
Inference):

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:17321",
  "inferenceGatewayAuthScheme": "bearer",
  "inferenceCredentialKind": "helper-script",
  "inferenceCredentialHelper": "/usr/local/bin/claude-desktop-cred",
  "inferenceCredentialHelperTtlSec": 300
}
```

| Flag | |
| --- | --- |
| `--header KEY=VALUE` | Add a header to the printed `headers` object (repeatable) |
| `--version` | Print the version |
| `-h`, `--help` | Print usage |

Headers are how a gateway gets its routing metadata; the helper itself is
vendor-neutral.

## Install

Download a binary from [Releases](https://github.com/sleev-labs/claude-desktop-credential-helper/releases),
or build it:

```sh
cargo build --release
```

## Development

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

Built and maintained by [Sleev](https://sleev.ai), a context-management
gateway for coding agents. It works with any gateway Claude Desktop can be
pointed at.
