use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::context::{ChatMessage, GenerationRequest};
use crate::error::{Error, Result};
use crate::settings::{LlmProfile, ProviderKind};
use crate::stream::{anthropic_sse_delta, openai_sse_delta, SseLineParser};

#[derive(Clone)]
pub struct OpenAiCompatClient {
    pub profile: LlmProfile,
    pub api_key: Option<String>,
}

// Manual Debug: never print the API key.
impl std::fmt::Debug for OpenAiCompatClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiCompatClient")
            .field("profile", &self.profile)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl OpenAiCompatClient {
    pub fn new(profile: LlmProfile, api_key: Option<String>) -> Self {
        Self { profile, api_key }
    }

    pub fn complete(&self, req: &GenerationRequest) -> Result<String> {
        match self.profile.provider {
            ProviderKind::Anthropic => self.complete_anthropic(req),
            _ => self.complete_openai(req),
        }
    }

    /// Stream a completion; `on_token` is called for each text delta.
    /// Returns the full accumulated text. Honours `cancel`.
    pub fn stream_complete(
        &self,
        req: &GenerationRequest,
        cancel: &AtomicBool,
        mut on_token: impl FnMut(&str),
    ) -> Result<String> {
        match self.profile.provider {
            ProviderKind::Anthropic => self.stream_anthropic(req, cancel, &mut on_token),
            _ => self.stream_openai(req, cancel, &mut on_token),
        }
    }

    fn build_openai_messages(&self, req: &GenerationRequest) -> Vec<ApiMessage> {
        let mut messages = Vec::new();
        messages.push(ApiMessage {
            role: "system".into(),
            content: req.system.clone(),
        });
        for m in &req.messages {
            messages.push(ApiMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            });
        }
        messages
    }

    fn complete_openai(&self, req: &GenerationRequest) -> Result<String> {
        if self.profile.provider != ProviderKind::Ollama && self.api_key.is_none() {
            return Err(Error::NoApiKey(self.profile.name.clone()));
        }

        let body = OpenAiRequest {
            model: self.profile.model.clone(),
            messages: self.build_openai_messages(req),
            // gpt-5 / o-series reject non-default temperature — omit for those.
            temperature: temperature_for_model(&self.profile.model),
            stream: None,
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        let url = format!(
            "{}/chat/completions",
            self.profile.base_url.trim_end_matches('/')
        );
        let mut builder = client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }

        let key = self.api_key.as_deref();
        let resp = builder.send().map_err(|e| e.without_url())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.without_url())?;
        if !status.is_success() {
            return Err(Error::api_redacted(format!("{status}: {text}"), key));
        }
        let parsed: OpenAiResponse = serde_json::from_str(&text)
            .map_err(|e| Error::api_redacted(format!("parse: {e}; body={text}"), key))?;
        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| Error::Api("empty choices".into()))
    }

    fn stream_openai(
        &self,
        req: &GenerationRequest,
        cancel: &AtomicBool,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String> {
        if self.profile.provider != ProviderKind::Ollama && self.api_key.is_none() {
            return Err(Error::NoApiKey(self.profile.name.clone()));
        }

        let body = OpenAiRequest {
            model: self.profile.model.clone(),
            messages: self.build_openai_messages(req),
            temperature: temperature_for_model(&self.profile.model),
            stream: Some(true),
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()?;

        let url = format!(
            "{}/chat/completions",
            self.profile.base_url.trim_end_matches('/')
        );
        let mut builder = client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }

        let key = self.api_key.as_deref();
        let mut resp = builder.send().map_err(|e| e.without_url())?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().map_err(|e| e.without_url())?;
            return Err(Error::api_redacted(format!("{status}: {text}"), key));
        }

        let mut acc = String::new();
        let mut parser = SseLineParser::new();
        let mut buf = [0u8; 2048];
        let mut done = false;
        loop {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            let n = resp.read(&mut buf).map_err(Error::Io)?;
            if n == 0 {
                break;
            }
            let chunk = String::from_utf8_lossy(&buf[..n]);
            parser.push(&chunk, |data| {
                if done {
                    return;
                }
                match openai_sse_delta(data) {
                    Ok(Some(delta)) => {
                        acc.push_str(&delta);
                        on_token(&delta);
                    }
                    Ok(None) => {}
                    Err(()) => done = true,
                }
            });
            if done {
                break;
            }
        }
        if acc.is_empty() && !cancel.load(Ordering::SeqCst) {
            // Some local servers ignore stream=true — fall back.
            return self.complete_openai(req);
        }
        Ok(acc)
    }

    fn complete_anthropic(&self, req: &GenerationRequest) -> Result<String> {
        let key = self
            .api_key
            .as_ref()
            .ok_or_else(|| Error::NoApiKey(self.profile.name.clone()))?;

        let messages: Vec<ApiMessage> = req
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| ApiMessage {
                role: if m.role == "assistant" {
                    "assistant".into()
                } else {
                    "user".into()
                },
                content: m.content.clone(),
            })
            .collect();

        let body = AnthropicRequest {
            model: self.profile.model.clone(),
            max_tokens: 4096,
            system: req.system.clone(),
            messages,
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        let url = format!("{}/messages", self.profile.base_url.trim_end_matches('/'));
        let resp = client
            .post(&url)
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .map_err(|e| e.without_url())?;
        let status = resp.status();
        let text = resp.text().map_err(|e| e.without_url())?;
        if !status.is_success() {
            return Err(Error::api_redacted(format!("{status}: {text}"), Some(key)));
        }
        let parsed: AnthropicResponse = serde_json::from_str(&text)
            .map_err(|e| Error::api_redacted(format!("parse: {e}; body={text}"), Some(key)))?;
        let content = parsed
            .content
            .into_iter()
            .filter(|b| b.block_type == "text")
            .map(|b| b.text.unwrap_or_default())
            .collect::<Vec<_>>()
            .join("");
        Ok(content)
    }

    fn stream_anthropic(
        &self,
        req: &GenerationRequest,
        cancel: &AtomicBool,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<String> {
        let key = self
            .api_key
            .as_ref()
            .ok_or_else(|| Error::NoApiKey(self.profile.name.clone()))?;

        let messages: Vec<ApiMessage> = req
            .messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| ApiMessage {
                role: if m.role == "assistant" {
                    "assistant".into()
                } else {
                    "user".into()
                },
                content: m.content.clone(),
            })
            .collect();

        let body = AnthropicStreamRequest {
            model: self.profile.model.clone(),
            max_tokens: 4096,
            system: req.system.clone(),
            messages,
            stream: true,
        };

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()?;

        let url = format!("{}/messages", self.profile.base_url.trim_end_matches('/'));
        let mut resp = client
            .post(&url)
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .map_err(|e| e.without_url())?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().map_err(|e| e.without_url())?;
            return Err(Error::api_redacted(format!("{status}: {text}"), Some(key)));
        }

        let mut acc = String::new();
        let mut parser = SseLineParser::new();
        let mut buf = [0u8; 2048];
        loop {
            if cancel.load(Ordering::SeqCst) {
                break;
            }
            let n = resp.read(&mut buf).map_err(Error::Io)?;
            if n == 0 {
                break;
            }
            let chunk = String::from_utf8_lossy(&buf[..n]);
            parser.push(&chunk, |data| {
                if let Some(delta) = anthropic_sse_delta(data) {
                    acc.push_str(&delta);
                    on_token(&delta);
                }
            });
        }
        if acc.is_empty() && !cancel.load(Ordering::SeqCst) {
            return self.complete_anthropic(req);
        }
        Ok(acc)
    }

    pub fn chat(&self, system: &str, history: &[ChatMessage], user: &str) -> Result<String> {
        let mut messages = history.to_vec();
        messages.push(ChatMessage {
            role: "user".into(),
            content: user.to_string(),
        });
        self.complete(&GenerationRequest {
            system: system.to_string(),
            messages,
        })
    }

    pub fn transform_selection(
        &self,
        system: &str,
        instruction: &str,
        selection: &str,
    ) -> Result<String> {
        self.complete(&GenerationRequest {
            system: system.to_string(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: format!("{instruction}\n\n---\nTexte à transformer:\n\n{selection}"),
            }],
        })
    }

    pub fn continue_writing(
        &self,
        system: &str,
        draft_tail: &str,
        mode: ContinueMode,
    ) -> Result<String> {
        let prompt = match mode {
            ContinueMode::Sentence => {
                "Continue en écrivant UNE phrase suivante, cohérente avec le draft. \
                 Renvoie uniquement cette phrase, sans guillemets ni commentaire."
            }
            ContinueMode::Paragraph => {
                "Continue en écrivant UN paragraphe suivant. \
                 Renvoie uniquement ce paragraphe, sans commentaire."
            }
            ContinueMode::Continue => {
                "Continue l'écriture de façon naturelle (quelques phrases à un court paragraphe). \
                 Renvoie uniquement le texte à ajouter, sans commentaire."
            }
            ContinueMode::Generate => {
                "Génère la suite demandée. Renvoie uniquement le texte à insérer."
            }
        };
        self.complete(&GenerationRequest {
            system: system.to_string(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: format!("{prompt}\n\n---\nFin du draft:\n\n{draft_tail}"),
            }],
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ContinueMode {
    Sentence,
    Paragraph,
    Continue,
    Generate,
}

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

/// Some OpenAI models (gpt-5*, o1/o3/o4*) only accept the default temperature.
pub(crate) fn temperature_for_model(model: &str) -> Option<f32> {
    let m = model.to_ascii_lowercase();
    let leaf = m.rsplit('/').next().unwrap_or(&m);
    if leaf.starts_with("gpt-5")
        || leaf.starts_with('o') && leaf.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
    {
        None
    } else {
        Some(0.7)
    }
}

#[cfg(test)]
mod tests {
    use super::temperature_for_model;

    #[test]
    fn gpt5_and_o_series_omit_temperature() {
        assert_eq!(temperature_for_model("gpt-5"), None);
        assert_eq!(temperature_for_model("gpt-5-mini"), None);
        assert_eq!(temperature_for_model("GPT-5-nano"), None);
        assert_eq!(temperature_for_model("o1"), None);
        assert_eq!(temperature_for_model("o1-mini"), None);
        assert_eq!(temperature_for_model("o3"), None);
        assert_eq!(temperature_for_model("o3-mini"), None);
    }

    #[test]
    fn gpt4o_uses_default_temperature() {
        assert_eq!(temperature_for_model("gpt-4o"), Some(0.7));
        assert_eq!(temperature_for_model("gpt-4o-mini"), Some(0.7));
    }

    #[test]
    fn openrouter_style_gpt5_leaf_omits_temperature() {
        assert_eq!(temperature_for_model("org/gpt-5-mini"), None);
        assert_eq!(temperature_for_model("openai/gpt-5"), None);
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ApiMessage,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
}

#[derive(Serialize)]
struct AnthropicStreamRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ApiMessage>,
    stream: bool,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicBlock>,
}

#[derive(Deserialize)]
struct AnthropicBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}
