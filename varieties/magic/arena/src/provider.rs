//! Where the model actually gets asked.
//!
//! Pokémon put an `LlmProvider` trait in Rust and shipped only
//! `PlaceholderLlmProvider`, which returns `Err("LLM provider not
//! implemented")`; the working client lived in Python against a Python-side
//! mock of the game. The two halves were never joined, which is why that lane
//! is still labelled a scaffold. The trait here is the same good idea with the
//! real client attached.
//!
//! HTTP is done by shelling out to `curl` rather than by adding an HTTP client.
//! `engine`, `policies`, and `rav` are required to stay dependency-free and the
//! whole workspace has kept to roughly that, so pulling in a TLS stack for one
//! POST per open decision is a poor trade. The trait is the seam: swapping in a
//! real client later changes this file and nothing else.

use std::io::Write as _;
use std::path::PathBuf;
use std::process::Command;

/// Anything that can answer a prompt.
pub trait LlmProvider: Send + Sync {
    /// A stable name for the transcript and the result line.
    fn id(&self) -> String;

    /// # Errors
    ///
    /// Returns an error when the model cannot be reached or returns nothing
    /// usable. Callers treat this as a fallback, never as a pass.
    fn complete(&self, system: &str, user: &str) -> Result<String, ProviderError>;
}

#[derive(Clone, Debug)]
pub struct ProviderError(pub String);

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl std::error::Error for ProviderError {}

/// A provider that replays canned replies in order.
///
/// This is what the tests use, and it is why the whole seat can be exercised
/// without a network or a key. A seat that can only be tested against a live
/// model is a seat nobody runs in CI.
pub struct ScriptedProvider {
    replies: std::sync::Mutex<std::collections::VecDeque<String>>,
    /// Returned once the script runs out, so a test can drive an arbitrary
    /// number of decisions after the interesting ones.
    default: String,
}

impl ScriptedProvider {
    #[must_use]
    pub fn new(replies: Vec<String>, default: &str) -> Self {
        Self {
            replies: std::sync::Mutex::new(replies.into()),
            default: default.to_owned(),
        }
    }

    /// Always chooses the first option, which is always the safe one.
    #[must_use]
    pub fn always_first() -> Self {
        Self::new(Vec::new(), r#"{"choice": 0}"#)
    }
}

impl LlmProvider for ScriptedProvider {
    fn id(&self) -> String {
        "scripted".to_owned()
    }

    fn complete(&self, _system: &str, _user: &str) -> Result<String, ProviderError> {
        let mut replies = self
            .replies
            .lock()
            .map_err(|error| ProviderError(format!("scripted provider poisoned: {error}")))?;
        Ok(replies.pop_front().unwrap_or_else(|| self.default.clone()))
    }
}

/// Settings for an OpenAI-compatible chat-completions endpoint.
///
/// `OpenRouter`, `DeepSeek` direct, and most gateways all speak this shape, so the
/// model is a string and the endpoint is a string.
#[derive(Clone, Debug)]
pub struct HttpConfig {
    pub base_url: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    /// Seconds allowed for one request.
    pub timeout: u32,
    /// How many times a failed request is retried before the seat falls back.
    pub retries: u32,
    /// Environment variable holding the API key. The key is a secret and
    /// belongs in the environment; the *configuration* does not, and lives in
    /// the arena TOML instead.
    pub key_variable: String,
    /// Reasoning effort, for models that think before answering.
    ///
    /// This is not a quality dial, it is a survival one. A reasoning model
    /// spends its token budget on reasoning *first*, so `gpt-oss-20b` at 512
    /// tokens burns 489 of them thinking and returns `finish_reason=length`
    /// with an empty message. Measured: `low` answers in 5.4s, unset answers in
    /// 17.8s after 1699 reasoning tokens.
    pub reasoning_effort: Option<String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            base_url: "https://openrouter.ai/api/v1".to_owned(),
            model: String::new(),
            temperature: 0.3,
            // Deliberately generous. The old default of 512 was smaller than
            // what a reasoning model spends before it writes anything, which
            // made every consultation fail and turned the seat silently into
            // its fallback policy.
            max_tokens: 2048,
            timeout: 60,
            retries: 2,
            key_variable: "OPENROUTER_API_KEY".to_owned(),
            reasoning_effort: Some("low".to_owned()),
        }
    }
}

pub struct HttpProvider {
    config: HttpConfig,
    key: String,
}

impl HttpProvider {
    /// # Errors
    ///
    /// Returns an error when the model is unset or the key variable is absent,
    /// because both are mistakes worth failing on before a run starts rather
    /// than discovering as a fallback on every decision.
    pub fn new(config: HttpConfig) -> Result<Self, ProviderError> {
        if config.model.is_empty() {
            return Err(ProviderError(
                "no model set; pass --model or set `model` in the arena config".to_owned(),
            ));
        }
        let key = std::env::var(&config.key_variable).map_err(|_| {
            ProviderError(format!(
                "environment variable `{}` is not set",
                config.key_variable
            ))
        })?;
        Ok(Self { config, key })
    }

    fn body(&self, system: &str, user: &str) -> String {
        let mut request = serde_json::json!({
            "model": self.config.model,
            "temperature": self.config.temperature,
            "max_tokens": self.config.max_tokens,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });
        // Ignored by endpoints that do not reason, so it costs nothing to send
        // and is the difference between an answer and an empty message on the
        // ones that do.
        if let Some(effort) = &self.config.reasoning_effort
            && let Some(object) = request.as_object_mut()
        {
            object.insert(
                "reasoning".to_owned(),
                serde_json::json!({ "effort": effort }),
            );
        }
        request.to_string()
    }

    fn post(&self, body: &str) -> Result<String, ProviderError> {
        let scratch = Scratch::new()?;
        scratch.write("body.json", body)?;
        // curl reads the key from a file rather than from `-H` on the command
        // line, because process arguments are world-readable on this platform
        // and an API key does not belong in `ps` output.
        let config = format!(
            "url = \"{}/chat/completions\"\nheader = \"Authorization: Bearer {}\"\nheader = \"Content-Type: application/json\"\ndata = \"@{}\"\n",
            self.config.base_url,
            self.key,
            scratch.path("body.json").display()
        );
        scratch.write("curl.conf", &config)?;

        let output = Command::new("curl")
            .arg("--silent")
            .arg("--show-error")
            .arg("--max-time")
            .arg(self.config.timeout.to_string())
            .arg("--config")
            .arg(scratch.path("curl.conf"))
            .output()
            .map_err(|error| ProviderError(format!("could not run curl: {error}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ProviderError(format!("curl failed: {}", stderr.trim())));
        }
        String::from_utf8(output.stdout)
            .map_err(|error| ProviderError(format!("response was not utf-8: {error}")))
    }
}

impl LlmProvider for HttpProvider {
    fn id(&self) -> String {
        self.config.model.clone()
    }

    fn complete(&self, system: &str, user: &str) -> Result<String, ProviderError> {
        let body = self.body(system, user);
        let mut last = ProviderError("no attempt was made".to_owned());
        for attempt in 0..=self.config.retries {
            if attempt > 0 {
                std::thread::sleep(std::time::Duration::from_millis(400 * u64::from(attempt)));
            }
            match self.post(&body).and_then(|raw| content(&raw)) {
                Ok(text) => return Ok(text),
                Err(error) => last = error,
            }
        }
        Err(last)
    }
}

/// Pulls the assistant message out of a chat-completions response.
///
/// An API error comes back as HTTP 200 with an `error` object often enough that
/// reading it explicitly is worth the branch; otherwise the failure surfaces as
/// the much less useful "no choices in response".
fn content(raw: &str) -> Result<String, ProviderError> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|error| ProviderError(format!("response was not json: {error}")))?;
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        return Err(ProviderError(format!("api error: {message}")));
    }
    let choice = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .ok_or_else(|| ProviderError("no choices in response".to_owned()))?;

    let text = choice
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();

    // A reasoning model that runs out of budget returns HTTP 200, a well-formed
    // envelope, and an empty message. Reported as "no choices in response" this
    // reads as a transport problem and sends you looking at the network; it is
    // really `max_tokens` being smaller than the model's own reasoning. Say so,
    // because the seat silently degrades to its fallback either way and the
    // only difference is whether anyone can tell why.
    if text.trim().is_empty() {
        let finish = choice
            .get("finish_reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown");
        let reasoning = value
            .get("usage")
            .and_then(|usage| usage.get("completion_tokens_details"))
            .and_then(|details| details.get("reasoning_tokens"))
            .and_then(serde_json::Value::as_u64);
        return Err(ProviderError(match (finish, reasoning) {
            ("length", Some(tokens)) => format!(
                "empty message: the model spent all {tokens} of its token budget \
                 reasoning and never answered -- raise max_tokens or lower \
                 reasoning_effort"
            ),
            ("length", None) => "empty message truncated by max_tokens -- raise it".to_owned(),
            (other, _) => format!("empty message (finish_reason={other})"),
        }));
    }
    Ok(text.to_owned())
}

/// A private directory that removes itself.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new() -> Result<Self, ProviderError> {
        // The process id plus a monotonic counter is enough: this is a
        // per-request scratch directory, not a security boundary, and the
        // permissions below are what actually protect the key.
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let ordinal = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("rav-arena-{}-{ordinal}", std::process::id()));
        std::fs::create_dir_all(&root)
            .map_err(|error| ProviderError(format!("scratch directory: {error}")))?;
        Ok(Self { root })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn write(&self, name: &str, contents: &str) -> Result<(), ProviderError> {
        let path = self.path(name);
        let mut file = std::fs::File::create(&path)
            .map_err(|error| ProviderError(format!("{}: {error}", path.display())))?;
        restrict(&file)?;
        file.write_all(contents.as_bytes())
            .map_err(|error| ProviderError(format!("{}: {error}", path.display())))
    }
}

#[cfg(unix)]
fn restrict(file: &std::fs::File) -> Result<(), ProviderError> {
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|error| ProviderError(format!("could not restrict scratch file: {error}")))
}

#[cfg(not(unix))]
const fn restrict(_file: &std::fs::File) -> Result<(), ProviderError> {
    Ok(())
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Best effort: the key is in here, but a failure to clean up must not
        // take down a match that is otherwise fine.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normal_response_yields_its_content() {
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":"{\"choice\": 1}"}}]}"#;
        assert_eq!(content(raw).unwrap(), r#"{"choice": 1}"#);
    }

    #[test]
    fn an_api_error_is_reported_as_itself() {
        // These arrive with HTTP 200. Without this branch the operator sees
        // "no choices in response" and goes looking in the wrong place.
        let raw = r#"{"error":{"message":"model not found","code":404}}"#;
        let error = content(raw).unwrap_err();
        assert!(error.0.contains("model not found"), "{}", error.0);
    }

    #[test]
    fn a_scripted_provider_replays_then_falls_back_to_its_default() {
        let provider =
            ScriptedProvider::new(vec![r#"{"choice": 2}"#.to_owned()], r#"{"choice": 0}"#);
        assert_eq!(provider.complete("", "").unwrap(), r#"{"choice": 2}"#);
        assert_eq!(provider.complete("", "").unwrap(), r#"{"choice": 0}"#);
    }

    #[test]
    fn a_reasoning_model_that_never_answered_says_exactly_that() {
        // The shape gpt-oss-20b returns at max_tokens=512: HTTP 200, a
        // well-formed envelope, an empty message, and the whole budget spent
        // thinking. Diagnosed as a transport error this costs an afternoon.
        let raw = r#"{"choices":[{"finish_reason":"length","message":{"role":"assistant","content":""}}],
                      "usage":{"completion_tokens":512,"completion_tokens_details":{"reasoning_tokens":489}}}"#;
        let Err(error) = content(raw) else {
            panic!("an empty message must not be treated as a reply");
        };
        assert!(error.0.contains("489"), "{}", error.0);
        assert!(error.0.contains("max_tokens"), "{}", error.0);
    }

    #[test]
    fn a_null_content_is_not_read_as_an_empty_answer() {
        let raw = r#"{"choices":[{"finish_reason":"stop","message":{"content":null}}]}"#;
        assert!(content(raw).is_err());
    }

    #[test]
    fn reasoning_effort_is_sent_when_configured_and_omitted_when_not() {
        let request = |effort: Option<&str>| {
            let provider = HttpProvider {
                config: HttpConfig {
                    model: "m".to_owned(),
                    reasoning_effort: effort.map(str::to_owned),
                    ..HttpConfig::default()
                },
                key: "unused".to_owned(),
            };
            serde_json::from_str::<serde_json::Value>(&provider.body("s", "u")).unwrap()
        };
        assert_eq!(request(Some("low"))["reasoning"]["effort"], "low");
        assert!(request(None).get("reasoning").is_none());
    }

    #[test]
    fn an_unset_model_fails_before_a_run_starts() {
        let Err(error) = HttpProvider::new(HttpConfig::default()) else {
            panic!("a provider with no model must not be constructible");
        };
        assert!(error.0.contains("no model set"), "{}", error.0);
    }
}
