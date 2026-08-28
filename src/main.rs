//! Credential helper for Claude Desktop's third-party inference mode.
//!
//! Reads the OAuth token stored locally by the `claude` CLI and prints it in
//! the `inferenceCredentialHelper` contract format:
//! `{"token": "...", "headers": {...}}` on stdout, exit 0. Any failure exits
//! non-zero with stdout untouched; guidance goes to stderr only when a user
//! is present (`CLAUDE_HELPER_CONTEXT`).

mod credentials;
mod refresh;
mod store;

use std::fmt;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

/// Whether stderr will be read. Claude Desktop names the contexts that must
/// stay silent; any other value, or none, means diagnostics are wanted.
fn interactive() -> bool {
    !matches!(
        std::env::var("CLAUDE_HELPER_CONTEXT").as_deref(),
        Ok("mid-session-refresh" | "scheduled-task" | "background")
    )
}

enum Failure {
    Store(store::StoreError),
    Parse {
        location: String,
        error: credentials::ParseError,
    },
    Expired,
    Refresh(refresh::RefreshError),
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(store::StoreError::NotFound { location }) => write!(
                f,
                "no Claude Code credentials at {location}; log in with the `claude` CLI \
                 (subscription account) and retry"
            ),
            Self::Store(error) => write!(f, "{error}"),
            Self::Parse { location, error } => {
                write!(f, "invalid credential store at {location}: {error}")
            }
            Self::Expired => write!(
                f,
                "the stored Claude Code token has expired and carries no refresh token; \
                 log in again with the `claude` CLI"
            ),
            Self::Refresh(error) => write!(
                f,
                "could not refresh the stored Claude Code token: {error}; \
                 log in again with the `claude` CLI"
            ),
        }
    }
}

struct Outcome {
    access_token: String,
    warning: Option<String>,
}

fn obtain(now_ms: i64) -> Result<Outcome, Failure> {
    let payload = store::read().map_err(Failure::Store)?;
    let creds = credentials::parse(&payload.json).map_err(|error| Failure::Parse {
        location: payload.source.clone(),
        error,
    })?;
    if !creds.is_expired(now_ms) {
        return Ok(Outcome {
            access_token: creds.access_token,
            warning: None,
        });
    }

    let Some(refresh_token) = creds.refresh_token else {
        return Err(Failure::Expired);
    };
    let refreshed = match refresh::refresh(&refresh_token, now_ms) {
        Ok(refreshed) => refreshed,
        // A concurrent `claude` run may have rotated the pair already, which
        // both invalidates our grant and leaves a usable token behind.
        Err(error) => {
            return match reread(now_ms) {
                Some(access_token) => Ok(Outcome {
                    access_token,
                    warning: None,
                }),
                None => Err(Failure::Refresh(error)),
            };
        }
    };

    let document = credentials::patch(
        &payload.json,
        &refreshed.access_token,
        &refreshed.refresh_token,
        refreshed.expires_at,
    )
    .map_err(|error| Failure::Parse {
        location: payload.source,
        error,
    })?;
    // A token we cannot persist still works until it expires, so the run
    // succeeds and only warns.
    let warning = store::write(&document)
        .err()
        .map(|error| format!("the refreshed token could not be saved: {error}"));

    Ok(Outcome {
        access_token: refreshed.access_token,
        warning,
    })
}

fn reread(now_ms: i64) -> Option<String> {
    let payload = store::read().ok()?;
    let creds = credentials::parse(&payload.json).ok()?;
    (!creds.is_expired(now_ms)).then_some(creds.access_token)
}

fn render_output(token: &str) -> String {
    serde_json::json!({ "token": token, "headers": {} }).to_string()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

fn run() -> ExitCode {
    match obtain(now_ms()) {
        Ok(outcome) => {
            if let Some(warning) = outcome.warning
                && interactive()
            {
                eprintln!("claude-desktop-cred: {warning}");
            }
            println!("{}", render_output(&outcome.access_token));
            ExitCode::SUCCESS
        }
        Err(failure) => {
            if interactive() {
                eprintln!("claude-desktop-cred: {failure}");
            }
            ExitCode::FAILURE
        }
    }
}

const USAGE: &str = "\
Usage: claude-desktop-cred [--version] [--help]

Prints the Claude Code OAuth token in Claude Desktop's
inferenceCredentialHelper format. Claude Desktop runs it with no arguments;
routing headers belong in Desktop's own inferenceCustomHeaders.";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None => run(),
        Some("--version") => {
            println!("claude-desktop-cred {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("-h" | "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("claude-desktop-cred: unknown argument '{other}'\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_contract_json() {
        assert_eq!(render_output("tok"), r#"{"headers":{},"token":"tok"}"#);
    }

    #[test]
    fn escapes_the_token() {
        assert_eq!(render_output("a\"b"), r#"{"headers":{},"token":"a\"b"}"#);
    }
}
