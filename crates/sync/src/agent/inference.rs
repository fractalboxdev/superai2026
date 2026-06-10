//! Inference backend (spec 02 §7, spec 04 §3) — trait-based, swapped by config.
//!
//! All production backends are compiled in and selected at RUNTIME:
//!
//! - **Vercel AI Gateway** (default cloud) — OpenAI-compatible
//!   `https://ai-gateway.vercel.sh/v1`, `AI_GATEWAY_API_KEY`, routing to
//!   Claude.
//! - **AWS Bedrock** — Converse API via `aws-sdk-bedrockruntime` (standard
//!   AWS credential chain).
//! - **LM Studio** — OpenAI-compatible on-host endpoint (`LM_STUDIO_URL`,
//!   default `http://localhost:1234/v1`) — the on-prem/offline mode.
//! - **Stub** — deterministic, no-LLM; the no-credential fallback that keeps
//!   Flow D's offline guarantee (structured query + redaction need no LLM).
//!
//! Inference always runs over already-permitted, capability-redacted content —
//! redaction happens in the brain query layer before any prompt is built.

use anyhow::{Context, Result};
use serde_json::json;

use crate::config::InferenceBackend;

pub trait Inference: Send + Sync {
    fn name(&self) -> &str;
    /// Complete a prompt over already-permitted, capability-redacted content.
    fn complete(&self, prompt: &str) -> Result<String>;
}

/// Deterministic, no-LLM backend (offline guarantee, used by tests).
pub struct StubInference;

impl Inference for StubInference {
    fn name(&self) -> &str {
        "stub"
    }
    fn complete(&self, prompt: &str) -> Result<String> {
        // A faithful "summarize" that never invents data: echo the salient line.
        let line = prompt
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("(no input)");
        Ok(format!("[stub-inference] {line}"))
    }
}

/// OpenAI-compatible chat-completions client — serves both the Vercel AI
/// Gateway and LM Studio (same wire shape, different base URL/key).
pub struct OpenAiCompat {
    label: &'static str,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl OpenAiCompat {
    pub fn gateway(api_key: String) -> Self {
        Self {
            label: "vercel-ai-gateway",
            base_url: std::env::var("AI_GATEWAY_URL")
                .unwrap_or_else(|_| "https://ai-gateway.vercel.sh/v1".into()),
            api_key: Some(api_key),
            model: std::env::var("CONTEXTFUL_MODEL")
                .unwrap_or_else(|_| "anthropic/claude-sonnet-4.5".into()),
        }
    }

    pub fn lm_studio() -> Self {
        let base_url =
            std::env::var("LM_STUDIO_URL").unwrap_or_else(|_| "http://localhost:1234/v1".into());
        let model = std::env::var("LM_STUDIO_MODEL")
            .ok()
            .or_else(|| Self::first_model(&base_url))
            .unwrap_or_else(|| "local".into());
        Self {
            label: "lm-studio",
            base_url,
            api_key: None,
            model,
        }
    }

    /// LM Studio: ask the server which model is loaded.
    fn first_model(base_url: &str) -> Option<String> {
        let resp: serde_json::Value = ureq::get(&format!("{base_url}/models"))
            .timeout(std::time::Duration::from_secs(3))
            .call()
            .ok()?
            .into_json()
            .ok()?;
        resp.get("data")?
            .as_array()?
            .first()?
            .get("id")?
            .as_str()
            .map(String::from)
    }
}

impl Inference for OpenAiCompat {
    fn name(&self) -> &str {
        self.label
    }

    fn complete(&self, prompt: &str) -> Result<String> {
        let mut req = ureq::post(&format!("{}/chat/completions", self.base_url))
            .set("content-type", "application/json")
            .timeout(std::time::Duration::from_secs(120));
        if let Some(key) = &self.api_key {
            req = req.set("authorization", &format!("Bearer {key}"));
        }
        let resp: serde_json::Value = req
            .send_json(json!({
                "model": self.model,
                "messages": [ { "role": "user", "content": prompt } ],
            }))
            .with_context(|| format!("{} chat/completions failed", self.label))?
            .into_json()
            .context("chat/completions response was not JSON")?;
        resp.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|t| t.as_str())
            .map(|s| s.trim().to_string())
            .ok_or_else(|| anyhow::anyhow!("no completion content in response"))
    }
}

/// AWS Bedrock Converse backend. The async SDK runs on a dedicated thread +
/// runtime because `complete` is called from sync contexts that may already
/// live inside the binary's tokio runtime.
pub struct Bedrock {
    model: String,
}

impl Bedrock {
    pub fn new() -> Self {
        Self {
            model: std::env::var("CONTEXTFUL_BEDROCK_MODEL")
                .unwrap_or_else(|_| "us.anthropic.claude-haiku-4-5-20251001-v1:0".into()),
        }
    }
}

impl Default for Bedrock {
    fn default() -> Self {
        Self::new()
    }
}

impl Inference for Bedrock {
    fn name(&self) -> &str {
        "bedrock"
    }

    fn complete(&self, prompt: &str) -> Result<String> {
        let model = self.model.clone();
        let prompt = prompt.to_string();
        std::thread::spawn(move || -> Result<String> {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            rt.block_on(async move {
                use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message};
                let aws = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
                let client = aws_sdk_bedrockruntime::Client::new(&aws);
                let out = client
                    .converse()
                    .model_id(&model)
                    .messages(
                        Message::builder()
                            .role(ConversationRole::User)
                            .content(ContentBlock::Text(prompt))
                            .build()
                            .map_err(|e| anyhow::anyhow!("{e}"))?,
                    )
                    .send()
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "bedrock converse: {}",
                            aws_sdk_bedrockruntime::error::DisplayErrorContext(e)
                        )
                    })?;
                let text = out
                    .output()
                    .and_then(|o| o.as_message().ok())
                    .map(|m| {
                        m.content()
                            .iter()
                            .filter_map(|c| c.as_text().ok())
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
                if text.is_empty() {
                    anyhow::bail!("bedrock returned no text content");
                }
                Ok(text)
            })
        })
        .join()
        .map_err(|_| anyhow::anyhow!("bedrock worker thread panicked"))?
    }
}

/// Select the inference backend from config (see [`InferenceBackend`] for the
/// runtime auto-detection order).
pub fn from_config(backend: InferenceBackend) -> Box<dyn Inference> {
    match backend {
        InferenceBackend::Stub => Box::new(StubInference),
        InferenceBackend::Gateway => match crate::config::nonempty_env("AI_GATEWAY_API_KEY") {
            Some(key) => Box::new(OpenAiCompat::gateway(key)),
            None => {
                tracing::warn!("AI_GATEWAY_API_KEY missing — falling back to stub (offline)");
                Box::new(StubInference)
            }
        },
        InferenceBackend::Bedrock => Box::new(Bedrock::new()),
        InferenceBackend::LmStudio => Box::new(OpenAiCompat::lm_studio()),
    }
}
