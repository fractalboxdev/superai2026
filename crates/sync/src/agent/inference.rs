//! Inference backend (spec 02 §7, spec 04 §3) — trait-based, swapped by config.
//!
//! The default `StubInference` is deterministic and needs no network, so the
//! scaffold compiles and the offline guarantee holds. Bedrock (`aws-sdk-
//! bedrockruntime` Converse) and LM Studio (`async-openai`) are the production
//! backends, gated behind features; selecting them without the feature falls
//! back to the stub with a warning.

use anyhow::Result;

use crate::config::InferenceBackend;

pub trait Inference: Send + Sync {
    fn name(&self) -> &str;
    /// Complete a prompt over already-permitted, capability-redacted content.
    fn complete(&self, prompt: &str) -> Result<String>;
}

/// Deterministic, no-LLM backend.
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

/// Select the inference backend from config.
pub fn from_config(backend: InferenceBackend) -> Box<dyn Inference> {
    match backend {
        InferenceBackend::Stub => Box::new(StubInference),
        InferenceBackend::Bedrock => {
            tracing::warn!(
                "Bedrock backend not compiled (needs `bedrock` feature + creds) — using stub"
            );
            Box::new(StubInference)
        }
        InferenceBackend::LmStudio => {
            tracing::warn!(
                "LM Studio backend not compiled (needs `lmstudio` feature) — using stub"
            );
            Box::new(StubInference)
        }
    }
}
