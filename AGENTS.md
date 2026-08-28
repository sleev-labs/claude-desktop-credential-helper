# claude-desktop-credential-helper

A single Rust binary, `claude-desktop-cred`, that prints the local Claude Code
OAuth token in Claude Desktop's `inferenceCredentialHelper` format.

## The contract

Claude Desktop runs the configured executable with no arguments and reads
stdout.

- Exit 0 means stdout holds the credential: either a bare token, or
  `{"token": "...", "headers": {...}}`. We always print the JSON form so
  `--header` values can ride along. Everything printed to stdout on exit 0 is
  treated as the credential — never write anything else there.
- Any non-zero exit is a failure; stdout is ignored and stderr is diagnostics.
- `CLAUDE_HELPER_CONTEXT` says who is waiting: `interactive` and `setup-test`
  mean a person will read stderr; `mid-session-refresh`, `scheduled-task` and
  `background` must stay silent, never prompt, and finish fast — Desktop kills
  a mid-session refresh after 20 seconds.
- Output is cached for `inferenceCredentialHelperTtlSec`, so every run must
  return a credential valid for at least that long.

## Credential store

Written by the `claude` CLI; we read and (on refresh) write it back.

- macOS: Keychain generic password, service `Claude Code-credentials`.
- Linux and Windows: `<CLAUDE_CONFIG_DIR or ~/.claude>/.credentials.json`.
- Shape: `{"claudeAiOauth": {"accessToken", "refreshToken", "expiresAt", …}}`,
  where `expiresAt` is epoch milliseconds.

Refreshing posts a `refresh_token` grant to Anthropic's OAuth token endpoint
with Claude Code's public client id, then patches the token fields back into
the stored document, leaving every other key alone.

## Rules

- Keep the dependency tree tiny (serde and serde_json today). The value of
  this tool is that a reader can audit it in one sitting; shell out to
  `curl`(1) and `security`(1) rather than pulling in an HTTP or keychain
  stack.
- No telemetry, no logging of the token, no network calls beyond the token
  endpoint.
- Never put a secret in argv — other processes can read it. Request bodies go
  over stdin.
- Stay vendor-neutral: gateway-specific values arrive through `--header`, not
  in the code.
- Comment only what is genuinely unintuitive.

## Layout

- `src/main.rs` — orchestration, failure classification, exit codes
  (0 credential, 1 failure, 2 usage).
- `src/cli.rs` — argument parsing.
- `src/credentials.rs` — store document model, expiry, and the patch applied
  after a refresh.
- `src/store.rs` — platform read and write (Keychain on macOS, file
  elsewhere).
- `src/refresh.rs` — the OAuth refresh grant.
  `CLAUDE_DESKTOP_CRED_TOKEN_URL` overrides the endpoint for tests.
- `tests/refresh.rs` — end-to-end run against a local token endpoint.

## Quality gate

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
