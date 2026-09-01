# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

- Build: `cargo build` (release: `cargo build --release`)
- All tests: `cargo test`
- One test: `cargo test <name>`; e2e suite only: `cargo test --test refresh`
- Lint/format: `cargo clippy` and `cargo fmt` (toolchain pinned to 1.94.0 in rust-toolchain.toml, which also declares musl targets for CI cross-builds)

## CI and release

- `.github/workflows/tests.yml` — fmt + clippy, and `cargo test` on Linux, macOS, and Windows. Runs on PRs and pushes to `main`, and is called by the release workflow as a gate.
- `.github/workflows/release.yml` — `prepare` (tag must equal the `Cargo.toml` version) → `test` → `build` (six targets, matching the sleev CLI) → `publish` (tags only: manifest via `scripts/build-release-manifest.mjs`, provenance attestation, upload to the sleev updates bucket under `claude-desktop-cred/`). `workflow_dispatch` is a build-only dry run.
- The manifest shape mirrors sleev's `cli/scripts/build-release-manifest.mjs` so the sleev CLI parses it with the same code it uses for gateway releases; keep the two in sync.

## What this is

A single small binary, `claude-desktop-cred`, used as Claude Desktop's `inferenceCredentialHelper`: it prints the Claude Code CLI's stored OAuth token as `{"token": "...", "headers": {}}` on stdout with exit 0. Any failure exits non-zero with stdout untouched. Only dependencies are serde/serde_json — HTTP and keychain access deliberately shell out to `curl` and `security(1)` instead of linking libraries.

## Architecture

Flow in `src/main.rs::obtain`: read store → parse → if expired, refresh → patch + persist. Two deliberate subtleties:

- If the refresh grant is rejected, the store is re-read before failing — a concurrent `claude` run may have rotated the token pair (invalidating our grant but leaving a fresh token behind).
- A refreshed token that cannot be persisted is still printed; that failure is a warning, not an error.

Modules:

- `store.rs` — platform credential storage behind `read()`/`write()`. macOS: Keychain service `"Claude Code-credentials"` via `security`. Elsewhere: `$CLAUDE_CONFIG_DIR/.credentials.json` (fallback `~/.claude/.credentials.json`), written atomically via temp file + rename, mode 0600. On macOS only, `CLAUDE_DESKTOP_CRED_STORE_FILE` swaps the Keychain for a file at that path; it exists for the e2e tests, which cannot seed a Keychain, and is compiled out elsewhere.
- `credentials.rs` — parses the store's `claudeAiOauth` section. Expiry uses a 300s margin matching the recommended `inferenceCredentialHelperTtlSec`, so a cached token stays valid for its whole cache lifetime. `patch()` rewrites only the token fields, preserving every other key the `claude` CLI owns.
- `refresh.rs` — OAuth refresh grant via `curl` subprocess (body over stdin, never argv; 15s timeout because Desktop kills the helper at 20s). Endpoint overridable with `CLAUDE_DESKTOP_CRED_TOKEN_URL` for tests.

## Conventions

- stdout is contract-only; diagnostics go to stderr, and only when interactive — `CLAUDE_HELPER_CONTEXT` values `mid-session-refresh`, `scheduled-task`, `background` must stay silent (see `interactive()` in main.rs).
- Secrets never go on argv (visible via `ps`); pass over stdin. The one unavoidable exception (`security add-generic-password -w`) is commented in store.rs.
- `tests/refresh.rs` is the end-to-end suite: it spins up a local TCP token endpoint and runs the real binary against a temp store.
