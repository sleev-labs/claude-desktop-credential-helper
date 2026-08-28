//! Credential helper for Claude Desktop's third-party inference mode.
//!
//! Reads the OAuth token stored locally by the `claude` CLI and prints it in
//! the `inferenceCredentialHelper` contract format:
//! `{"token": "...", "headers": {...}}` on stdout, exit 0. Any failure exits
//! non-zero with stdout untouched; guidance goes to stderr only when a user
//! is present (`CLAUDE_HELPER_CONTEXT`).

mod cli;
mod credentials;
mod store;

use std::collections::BTreeMap;
use std::fmt;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelperContext {
    Interactive,
    Silent,
}

impl HelperContext {
    fn from_env() -> Self {
        match std::env::var("CLAUDE_HELPER_CONTEXT").as_deref() {
            Ok("mid-session-refresh" | "scheduled-task" | "background") => Self::Silent,
            // `interactive`, `setup-test`, unset, or unknown future values.
            _ => Self::Interactive,
        }
    }
}

enum Failure {
    Store(store::StoreError),
    Parse {
        location: String,
        error: credentials::ParseError,
    },
    Expired,
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
                "the stored Claude Code token has expired; run `claude` once to refresh it"
            ),
        }
    }
}

fn obtain(now_ms: i64) -> Result<credentials::OauthCredentials, Failure> {
    let payload = store::read().map_err(Failure::Store)?;
    let creds = credentials::parse(&payload.json).map_err(|error| Failure::Parse {
        location: payload.source,
        error,
    })?;
    if creds.is_expired(now_ms) {
        return Err(Failure::Expired);
    }
    Ok(creds)
}

fn render_output(token: &str, headers: &BTreeMap<String, String>) -> String {
    serde_json::json!({ "token": token, "headers": headers }).to_string()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

fn run(args: &cli::Args) -> ExitCode {
    match obtain(now_ms()) {
        Ok(creds) => {
            println!("{}", render_output(&creds.access_token, &args.headers));
            ExitCode::SUCCESS
        }
        Err(failure) => {
            // The helper contract: silent contexts must fail fast with no output.
            if HelperContext::from_env() == HelperContext::Interactive {
                eprintln!("claude-desktop-cred: {failure}");
            }
            ExitCode::FAILURE
        }
    }
}

fn main() -> ExitCode {
    match cli::parse(std::env::args().skip(1)) {
        Ok(cli::Cli::Run(args)) => run(&args),
        Ok(cli::Cli::Version) => {
            println!("claude-desktop-cred {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(cli::Cli::Help) => {
            println!("{}", cli::USAGE);
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("claude-desktop-cred: {message}\n\n{}", cli::USAGE);
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_contract_json() {
        let mut headers = BTreeMap::new();
        headers.insert("sleev-provider".to_owned(), "anthropic".to_owned());
        assert_eq!(
            render_output("tok", &headers),
            r#"{"headers":{"sleev-provider":"anthropic"},"token":"tok"}"#
        );
    }

    #[test]
    fn renders_empty_headers_as_object() {
        assert_eq!(
            render_output("tok", &BTreeMap::new()),
            r#"{"headers":{},"token":"tok"}"#
        );
    }
}
