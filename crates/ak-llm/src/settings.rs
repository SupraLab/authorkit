use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const SERVICE: &str = "authorkit-rust";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    #[default]
    OpenAiCompatible,
    Anthropic,
    Ollama,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "OpenAI-compatible",
            Self::Anthropic => "Anthropic",
            Self::Ollama => "Ollama (local)",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Ollama => "http://127.0.0.1:11434/v1",
        }
    }

    pub fn default_model(self) -> &'static str {
        match self {
            Self::OpenAiCompatible => "gpt-5-mini",
            Self::Anthropic => "claude-sonnet-4-5",
            Self::Ollama => "llama3.2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProfile {
    pub name: String,
    #[serde(default)]
    pub provider: ProviderKind,
    pub base_url: String,
    pub model: String,
    /// Logical credential id (not a secret). Empty for Ollama.
    #[serde(default)]
    pub keyring_account: String,
}

impl Default for LlmProfile {
    fn default() -> Self {
        Self {
            name: "Par défaut".into(),
            provider: ProviderKind::OpenAiCompatible,
            base_url: ProviderKind::OpenAiCompatible.default_base_url().into(),
            model: ProviderKind::OpenAiCompatible.default_model().into(),
            keyring_account: "openai".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub active_profile: String,
    pub profiles: Vec<LlmProfile>,
}

impl Default for LlmSettings {
    fn default() -> Self {
        let p = LlmProfile::default();
        Self {
            active_profile: p.name.clone(),
            profiles: vec![
                p,
                LlmProfile {
                    name: "Ollama".into(),
                    provider: ProviderKind::Ollama,
                    base_url: ProviderKind::Ollama.default_base_url().into(),
                    model: ProviderKind::Ollama.default_model().into(),
                    keyring_account: String::new(),
                },
            ],
        }
    }
}

fn authorkit_home() -> PathBuf {
    if let Ok(p) = std::env::var("AUTHORKIT_HOME") {
        return PathBuf::from(p);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".authorkit-rust")
}

impl LlmSettings {
    pub fn path() -> PathBuf {
        authorkit_home().join("llm_settings.json")
    }

    fn secrets_dir() -> PathBuf {
        authorkit_home().join("secrets")
    }

    fn secret_file(account: &str) -> PathBuf {
        // Sanitize account for a filename.
        let safe: String = account
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        Self::secrets_dir().join(safe)
    }

    pub fn load() -> Self {
        let path = Self::path();
        let mut s = if path.exists() {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                serde_json::from_str(&raw).unwrap_or_default()
            } else {
                Self::default()
            }
        } else {
            Self::default()
        };
        s.normalize_keyring_accounts();
        s
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    pub fn active(&self) -> Option<&LlmProfile> {
        self.profiles
            .iter()
            .find(|p| p.name == self.active_profile)
            .or_else(|| self.profiles.first())
    }

    pub fn active_mut(&mut self) -> Option<&mut LlmProfile> {
        let name = self.active_profile.clone();
        if let Some(i) = self.profiles.iter().position(|p| p.name == name) {
            Some(&mut self.profiles[i])
        } else {
            self.profiles.first_mut()
        }
    }

    pub fn normalize_keyring_accounts(&mut self) {
        for p in &mut self.profiles {
            if let Some(id) = credential_account(p) {
                p.keyring_account = id;
            } else {
                p.keyring_account.clear();
            }
        }
    }

    pub fn set_api_key(&self, profile: &LlmProfile, key: &str) -> Result<()> {
        let key = key.trim();
        if key.is_empty() {
            return Err(Error::Keyring("clé vide".into()));
        }
        let account = credential_account(profile)
            .ok_or_else(|| Error::Keyring("ce fournisseur ne nécessite pas de clé".into()))?;

        // Primary: local file (reliable for unsigned macOS apps).
        write_secret_file(&account, key)?;

        // Best-effort Keychain mirror (may fail without user prompt / signing).
        if let Ok(entry) = keyring::Entry::new(SERVICE, &account) {
            let _ = entry.set_password(key);
        }

        match read_secret_file(&account)? {
            Some(stored) if stored == key => Ok(()),
            _ => Err(Error::Keyring(
                "impossible d’écrire la clé dans ~/.authorkit-rust/secrets".into(),
            )),
        }
    }

    pub fn get_api_key(&self, profile: &LlmProfile) -> Result<Option<String>> {
        for account in candidate_accounts(profile) {
            if let Some(k) = read_secret_file(&account)? {
                return Ok(Some(k));
            }
            // Migrate from Keychain → file if present.
            if let Ok(entry) = keyring::Entry::new(SERVICE, &account) {
                match entry.get_password() {
                    Ok(p) if !p.is_empty() => {
                        let _ = write_secret_file(&account, &p);
                        return Ok(Some(p));
                    }
                    _ => {}
                }
            }
        }
        Ok(None)
    }

    pub fn has_api_key(&self, profile: &LlmProfile) -> bool {
        matches!(self.get_api_key(profile), Ok(Some(_)))
    }

    pub fn ensure_dir() {
        let _ = std::fs::create_dir_all(Self::path().parent().unwrap_or_else(|| Path::new(".")));
        let _ = std::fs::create_dir_all(Self::secrets_dir());
    }
}

fn write_secret_file(account: &str, key: &str) -> Result<()> {
    let path = LlmSettings::secret_file(account);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn read_secret_file(account: &str) -> Result<Option<String>> {
    let path = LlmSettings::secret_file(account);
    match std::fs::read_to_string(&path) {
        Ok(raw) => {
            let s = raw.trim().to_string();
            if s.is_empty() {
                Ok(None)
            } else {
                Ok(Some(s))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Io(e)),
    }
}

/// Stable credential id shared by all profiles hitting the same provider endpoint.
pub fn credential_account(profile: &LlmProfile) -> Option<String> {
    if profile.provider == ProviderKind::Ollama {
        return None;
    }
    let url = profile.base_url.trim_end_matches('/').to_ascii_lowercase();
    let id = if url.contains("api.openai.com") {
        "openai"
    } else if url.contains("anthropic.com") {
        "anthropic"
    } else if url.contains("generativelanguage.googleapis.com")
        || url.contains("googleapis.com/v1beta/openai")
    {
        "gemini"
    } else if url.contains("mistral.ai") {
        "mistral"
    } else if url.contains("deepseek.com") {
        "deepseek"
    } else if url.contains("api.x.ai") || url.contains("//x.ai") {
        "xai"
    } else if url.contains("openrouter.ai") {
        "openrouter"
    } else if url.contains("groq.com") {
        "groq"
    } else if !profile.keyring_account.is_empty() {
        return Some(profile.keyring_account.clone());
    } else {
        let slug: String = profile
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        return Some(if slug.is_empty() {
            "default".into()
        } else {
            slug
        });
    };
    Some(id.into())
}

fn candidate_accounts(profile: &LlmProfile) -> Vec<String> {
    let mut out = Vec::new();
    let push = |out: &mut Vec<String>, s: String| {
        if !s.is_empty() && !out.iter().any(|x| x == &s) {
            out.push(s);
        }
    };
    if let Some(primary) = credential_account(profile) {
        push(&mut out, primary);
    }
    push(&mut out, profile.keyring_account.clone());
    let url = profile.base_url.to_ascii_lowercase();
    if url.contains("api.openai.com") {
        push(&mut out, "default".into());
        push(&mut out, "openai".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// `AUTHORKIT_HOME` / secrets paths are process-global; serialize tests that touch them.
    fn home_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn file_secret_roundtrip() {
        let _guard = home_lock();
        let dir = std::env::temp_dir().join(format!(
            "ak-llm-secrets-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::var_os("AUTHORKIT_HOME");
        std::env::set_var("AUTHORKIT_HOME", &dir);

        let account = "test-openai-roundtrip";
        write_secret_file(account, "sk-file-test").unwrap();
        assert_eq!(
            read_secret_file(account).unwrap().as_deref(),
            Some("sk-file-test")
        );
        let _ = std::fs::remove_file(LlmSettings::secret_file(account));

        match prev {
            Some(v) => std::env::set_var("AUTHORKIT_HOME", v),
            None => std::env::remove_var("AUTHORKIT_HOME"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn openai_profiles_share_credential_id() {
        let a = LlmProfile {
            name: "Par défaut".into(),
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-5-mini".into(),
            keyring_account: "default".into(),
        };
        let b = LlmProfile {
            name: "OpenAI".into(),
            provider: ProviderKind::OpenAiCompatible,
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-5-mini".into(),
            keyring_account: "openai".into(),
        };
        assert_eq!(credential_account(&a).as_deref(), Some("openai"));
        assert_eq!(credential_account(&b).as_deref(), Some("openai"));
    }

    fn profile(name: &str, provider: ProviderKind, base_url: &str, keyring: &str) -> LlmProfile {
        LlmProfile {
            name: name.into(),
            provider,
            base_url: base_url.into(),
            model: "m".into(),
            keyring_account: keyring.into(),
        }
    }

    #[test]
    fn credential_account_provider_hosts() {
        assert_eq!(
            credential_account(&profile(
                "Ollama",
                ProviderKind::Ollama,
                "http://127.0.0.1:11434/v1",
                ""
            )),
            None
        );
        assert_eq!(
            credential_account(&profile(
                "Claude",
                ProviderKind::Anthropic,
                "https://api.anthropic.com/v1",
                ""
            ))
            .as_deref(),
            Some("anthropic")
        );
        assert_eq!(
            credential_account(&profile(
                "Gemini",
                ProviderKind::OpenAiCompatible,
                "https://generativelanguage.googleapis.com/v1beta/openai",
                ""
            ))
            .as_deref(),
            Some("gemini")
        );
        assert_eq!(
            credential_account(&profile(
                "Mistral",
                ProviderKind::OpenAiCompatible,
                "https://api.mistral.ai/v1",
                ""
            ))
            .as_deref(),
            Some("mistral")
        );
        assert_eq!(
            credential_account(&profile(
                "DeepSeek",
                ProviderKind::OpenAiCompatible,
                "https://api.deepseek.com/v1",
                ""
            ))
            .as_deref(),
            Some("deepseek")
        );
        assert_eq!(
            credential_account(&profile(
                "xAI",
                ProviderKind::OpenAiCompatible,
                "https://api.x.ai/v1",
                ""
            ))
            .as_deref(),
            Some("xai")
        );
        assert_eq!(
            credential_account(&profile(
                "OpenRouter",
                ProviderKind::OpenAiCompatible,
                "https://openrouter.ai/api/v1",
                ""
            ))
            .as_deref(),
            Some("openrouter")
        );
        assert_eq!(
            credential_account(&profile(
                "Groq",
                ProviderKind::OpenAiCompatible,
                "https://api.groq.com/openai/v1",
                ""
            ))
            .as_deref(),
            Some("groq")
        );
    }

    #[test]
    fn credential_account_custom_url_fallbacks() {
        assert_eq!(
            credential_account(&profile(
                "My Proxy",
                ProviderKind::OpenAiCompatible,
                "https://llm.example.com/v1",
                "my-account"
            ))
            .as_deref(),
            Some("my-account")
        );
        assert_eq!(
            credential_account(&profile(
                "My Proxy",
                ProviderKind::OpenAiCompatible,
                "https://llm.example.com/v1",
                ""
            ))
            .as_deref(),
            Some("my-proxy")
        );
        assert_eq!(
            credential_account(&profile(
                "",
                ProviderKind::OpenAiCompatible,
                "https://llm.example.com/v1",
                ""
            ))
            .as_deref(),
            Some("default")
        );
    }

    #[test]
    fn provider_kind_defaults_and_labels() {
        assert_eq!(ProviderKind::default(), ProviderKind::OpenAiCompatible);
        for (kind, label, url, model) in [
            (
                ProviderKind::OpenAiCompatible,
                "OpenAI-compatible",
                "https://api.openai.com/v1",
                "gpt-5-mini",
            ),
            (
                ProviderKind::Anthropic,
                "Anthropic",
                "https://api.anthropic.com/v1",
                "claude-sonnet-4-5",
            ),
            (
                ProviderKind::Ollama,
                "Ollama (local)",
                "http://127.0.0.1:11434/v1",
                "llama3.2",
            ),
        ] {
            assert_eq!(kind.label(), label);
            assert_eq!(kind.default_base_url(), url);
            assert_eq!(kind.default_model(), model);
        }
        // Missing `provider` in JSON → serde default (OpenAiCompatible).
        let p: LlmProfile =
            serde_json::from_str(r#"{"name":"X","base_url":"http://x","model":"m"}"#).unwrap();
        assert_eq!(p.provider, ProviderKind::OpenAiCompatible);
        assert!(p.keyring_account.is_empty());
    }

    #[test]
    fn settings_load_save_roundtrip_under_authorkit_home() {
        let _guard = home_lock();
        let dir = std::env::temp_dir().join(format!(
            "ak-llm-home-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::var_os("AUTHORKIT_HOME");
        std::env::set_var("AUTHORKIT_HOME", &dir);

        // Missing file → built-in defaults (OpenAI + Ollama profiles).
        let defaults = LlmSettings::load();
        assert_eq!(defaults.profiles.len(), 2);
        assert_eq!(defaults.active_profile, "Par défaut");
        assert_eq!(
            defaults.active().unwrap().provider,
            ProviderKind::OpenAiCompatible
        );
        assert!(defaults
            .profiles
            .iter()
            .any(|p| p.provider == ProviderKind::Ollama));

        LlmSettings::ensure_dir();
        assert!(dir.join("secrets").is_dir());

        let mut custom = LlmSettings {
            active_profile: "Custom".into(),
            profiles: vec![
                LlmProfile {
                    name: "Custom".into(),
                    provider: ProviderKind::Anthropic,
                    base_url: ProviderKind::Anthropic.default_base_url().into(),
                    model: "claude-test".into(),
                    keyring_account: "stale".into(),
                },
                LlmProfile {
                    name: "Local".into(),
                    provider: ProviderKind::Ollama,
                    base_url: ProviderKind::Ollama.default_base_url().into(),
                    model: "llama".into(),
                    keyring_account: "should-clear".into(),
                },
            ],
        };
        custom.normalize_keyring_accounts();
        assert_eq!(custom.profiles[0].keyring_account, "anthropic");
        assert!(custom.profiles[1].keyring_account.is_empty());
        custom.save().unwrap();
        assert!(LlmSettings::path().starts_with(&dir));
        assert!(LlmSettings::path().exists());

        let loaded = LlmSettings::load();
        assert_eq!(loaded.active_profile, "Custom");
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.active().unwrap().model, "claude-test");
        assert_eq!(loaded.active().unwrap().provider, ProviderKind::Anthropic);

        // active_mut falls back to first profile when name is unknown.
        let mut loaded = loaded;
        loaded.active_profile = "missing".into();
        assert_eq!(loaded.active_mut().unwrap().name, "Custom");

        // Corrupt JSON → defaults again.
        std::fs::write(LlmSettings::path(), "{not-json").unwrap();
        let recovered = LlmSettings::load();
        assert_eq!(recovered.active_profile, "Par défaut");
        assert_eq!(recovered.profiles.len(), 2);

        match prev {
            Some(v) => std::env::set_var("AUTHORKIT_HOME", v),
            None => std::env::remove_var("AUTHORKIT_HOME"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_key_file_roundtrip_and_errors() {
        let _guard = home_lock();
        let dir = std::env::temp_dir().join(format!(
            "ak-llm-keys-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::var_os("AUTHORKIT_HOME");
        std::env::set_var("AUTHORKIT_HOME", &dir);
        LlmSettings::ensure_dir();

        let settings = LlmSettings::default();
        let openai = settings.profiles[0].clone();
        let ollama = settings.profiles[1].clone();

        assert!(settings.set_api_key(&openai, "   ").is_err());
        assert!(settings.set_api_key(&ollama, "sk-x").is_err());
        assert!(!settings.has_api_key(&openai));

        settings.set_api_key(&openai, "  sk-test-key  ").unwrap();
        assert_eq!(
            settings.get_api_key(&openai).unwrap().as_deref(),
            Some("sk-test-key")
        );
        assert!(settings.has_api_key(&openai));

        // Sanitized filename for odd account ids.
        let custom = profile(
            "Proxy",
            ProviderKind::OpenAiCompatible,
            "https://proxy.example/v1",
            "acc/with spaces!",
        );
        settings.set_api_key(&custom, "sk-custom").unwrap();
        assert_eq!(
            settings.get_api_key(&custom).unwrap().as_deref(),
            Some("sk-custom")
        );

        match prev {
            Some(v) => std::env::set_var("AUTHORKIT_HOME", v),
            None => std::env::remove_var("AUTHORKIT_HOME"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
