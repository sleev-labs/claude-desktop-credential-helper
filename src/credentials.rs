use serde::Deserialize;

/// Claude Desktop caches helper output for `inferenceCredentialHelperTtlSec`
/// (we recommend 300); refusing tokens that expire within the same window
/// keeps a cached token valid for its whole cache lifetime.
pub const EXPIRY_MARGIN_MS: i64 = 300_000;

#[derive(Debug, Deserialize)]
struct Document {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OauthCredentials>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OauthCredentials {
    pub access_token: String,
    /// Epoch milliseconds.
    #[serde(default)]
    pub expires_at: Option<i64>,
}

impl OauthCredentials {
    pub fn is_expired(&self, now_ms: i64) -> bool {
        self.expires_at
            .is_some_and(|at| at - now_ms <= EXPIRY_MARGIN_MS)
    }
}

#[derive(Debug)]
pub enum ParseError {
    Malformed(String),
    NoOauthCredentials,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(cause) => write!(f, "not the expected JSON document: {cause}"),
            Self::NoOauthCredentials => write!(
                f,
                "no subscription OAuth credentials in the store; \
                 log in with the `claude` CLI using a Claude subscription account"
            ),
        }
    }
}

pub fn parse(payload: &str) -> Result<OauthCredentials, ParseError> {
    let document: Document =
        serde_json::from_str(payload).map_err(|error| ParseError::Malformed(error.to_string()))?;
    document
        .claude_ai_oauth
        .ok_or(ParseError::NoOauthCredentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORE: &str = r#"{
        "claudeAiOauth": {
            "accessToken": "sk-ant-oat01-fake",
            "refreshToken": "sk-ant-ort01-fake",
            "expiresAt": 1000000,
            "scopes": ["user:inference"],
            "subscriptionType": "max",
            "rateLimitTier": "default"
        }
    }"#;

    #[test]
    fn parses_the_stored_shape() {
        let creds = parse(STORE).unwrap();
        assert_eq!(creds.access_token, "sk-ant-oat01-fake");
        assert_eq!(creds.expires_at, Some(1_000_000));
    }

    #[test]
    fn missing_oauth_section_is_a_distinct_error() {
        assert!(matches!(parse("{}"), Err(ParseError::NoOauthCredentials)));
    }

    #[test]
    fn malformed_json_is_reported() {
        assert!(matches!(parse("not json"), Err(ParseError::Malformed(_))));
    }

    #[test]
    fn expiry_applies_the_margin() {
        let creds = parse(STORE).unwrap();
        let expires_at = 1_000_000;
        assert!(!creds.is_expired(expires_at - EXPIRY_MARGIN_MS - 1));
        assert!(creds.is_expired(expires_at - EXPIRY_MARGIN_MS));
        assert!(creds.is_expired(expires_at + 1));
    }

    #[test]
    fn missing_expiry_never_expires() {
        let creds = parse(r#"{"claudeAiOauth": {"accessToken": "t"}}"#).unwrap();
        assert!(!creds.is_expired(i64::MAX));
    }
}
