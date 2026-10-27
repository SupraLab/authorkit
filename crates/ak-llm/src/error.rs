#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error: {0}")]
    Api(String),
    #[error("No API key configured for profile '{0}'")]
    NoApiKey(String),
    #[error("Keyring error: {0}")]
    Keyring(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Build an API error whose message can never contain the API key.
    ///
    /// Provider error bodies sometimes echo the credential (e.g. OpenAI's
    /// "Incorrect API key provided: sk-abc***xyz"); this is the only
    /// constructor that should be used for text coming from a remote server.
    pub fn api_redacted(message: impl AsRef<str>, api_key: Option<&str>) -> Self {
        Error::Api(redact_secrets(message.as_ref(), api_key))
    }
}

const MASK: &str = "[clé masquée]";

/// Remove API keys from `text`: the exact `known_key` if any, `Bearer <token>`
/// values, and tokens that look like provider keys (`sk-…`, `sk_…`, `gsk_…`, `xai-…`).
pub fn redact_secrets(text: &str, known_key: Option<&str>) -> String {
    let mut out = text.to_string();
    if let Some(k) = known_key.map(str::trim).filter(|k| k.len() >= 6) {
        out = out.replace(k, MASK);
    }

    let is_token_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '*' | '.');
    let mut res = String::with_capacity(out.len());
    let mut token = String::new();
    let mut after_bearer = false;
    let flush = |token: &mut String, res: &mut String, after_bearer: &mut bool| {
        if token.is_empty() {
            return;
        }
        let looks_like_key = token.len() >= 8
            && ["sk-", "sk_", "gsk_", "xai-"]
                .iter()
                .any(|p| token.starts_with(p));
        if looks_like_key || (*after_bearer && token.len() >= 8) {
            res.push_str(MASK);
        } else {
            res.push_str(token);
        }
        *after_bearer = token.eq_ignore_ascii_case("bearer");
        token.clear();
    };
    for c in out.chars() {
        if is_token_char(c) {
            token.push(c);
        } else {
            flush(&mut token, &mut res, &mut after_bearer);
            if !c.is_whitespace() && c != ':' {
                after_bearer = false;
            }
            res.push(c);
        }
    }
    flush(&mut token, &mut res, &mut after_bearer);
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_exact_key() {
        let key = "my-very-secret-token-123";
        let s = redact_secrets(&format!("401: bad key {key}!"), Some(key));
        assert!(!s.contains(key));
        assert!(s.contains(MASK));
    }

    #[test]
    fn masks_provider_style_keys_and_bearer() {
        let s = redact_secrets(
            "Incorrect API key provided: sk-proj-abc***xyz. Authorization: Bearer abcdef123456",
            None,
        );
        assert!(!s.contains("sk-proj"));
        assert!(!s.contains("abcdef123456"));
        assert!(s.contains("Incorrect API key provided"));
    }

    #[test]
    fn leaves_normal_text_alone() {
        let t = "429: rate limit exceeded; task-force and sk-8 stay";
        assert_eq!(redact_secrets(t, None), t);
    }

    #[test]
    fn api_redacted_constructor() {
        let e = Error::api_redacted("oops secret-key-9999", Some("secret-key-9999"));
        assert!(!e.to_string().contains("secret-key-9999"));
    }
}
