//! Arena configuration, read from a file rather than the environment.
//!
//! Which model to call, where to call it, and how much to spend are
//! configuration and live in `arena.toml`. Only the API key comes from the
//! environment, because a key is a secret and a secret is the one thing that
//! should not be in a file under version control.
//!
//! The reader is a flat `key = value` parser rather than a TOML dependency.
//! The whole workspace is close to dependency-free, this file has ten keys and
//! no nesting, and `sets/ravnica_city_of_guilds` already reads its deck index
//! the same way.

use std::path::Path;

use crate::provider::HttpConfig;

/// The default location, relative to the variety root.
pub const DEFAULT_PATH: &str = "arena.toml";

#[derive(Clone, Debug)]
pub struct ArenaConfig {
    pub http: HttpConfig,
    /// Consultations allowed per game per model seat.
    pub budget: u32,
    /// Seed pairs per deck. Each pair is two games with the seats swapped.
    pub pairs: u32,
    /// Keep every prompt and reply.
    pub record: bool,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            http: HttpConfig::default(),
            budget: 120,
            // Small on purpose. The ladder's 60 pairs across four decks is 480
            // games; at even 30 consultations a game that is a five-figure
            // number of model calls. Widen deliberately, not by default.
            pairs: 3,
            record: false,
        }
    }
}

impl ArenaConfig {
    /// Reads a config file, falling back to defaults for absent keys.
    ///
    /// A missing file is not an error: the defaults are usable for everything
    /// except the model name, which has no sensible default and is checked
    /// when the provider is built.
    ///
    /// # Errors
    ///
    /// Returns an error when the file exists but cannot be read, or when a
    /// numeric key does not parse. A malformed budget is worth stopping for;
    /// silently using the default would misreport what a run cost.
    pub fn load(path: &Path) -> Result<Self, String> {
        let mut config = Self::default();
        if !path.exists() {
            return Ok(config);
        }
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;

        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            let numeric = |name: &str| -> Result<u32, String> {
                value.parse::<u32>().map_err(|error| {
                    format!(
                        "{}:{}: `{name}` must be a nonnegative integer: {error}",
                        path.display(),
                        number + 1
                    )
                })
            };
            match key {
                "model" => value.clone_into(&mut config.http.model),
                "base_url" => value.clone_into(&mut config.http.base_url),
                "key_variable" => value.clone_into(&mut config.http.key_variable),
                "temperature" => {
                    config.http.temperature = value.parse::<f32>().map_err(|error| {
                        format!(
                            "{}:{}: `temperature` must be a number: {error}",
                            path.display(),
                            number + 1
                        )
                    })?;
                }
                "max_tokens" => config.http.max_tokens = numeric("max_tokens")?,
                "timeout" => config.http.timeout = numeric("timeout")?,
                "retries" => config.http.retries = numeric("retries")?,
                "budget" => config.budget = numeric("budget")?,
                "pairs" => config.pairs = numeric("pairs")?,
                "record" => config.record = value == "true",
                // `none` disables it; anything else is passed through, because
                // which efforts an endpoint accepts is the endpoint's business.
                "reasoning_effort" => {
                    config.http.reasoning_effort = if value == "none" {
                        None
                    } else {
                        Some(value.to_owned())
                    };
                }
                // An unknown key is a typo the operator should see rather than
                // a setting that silently did nothing.
                other => {
                    return Err(format!(
                        "{}:{}: unknown key `{other}`",
                        path.display(),
                        number + 1
                    ));
                }
            }
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("rav-arena-config-test-{name}.toml"));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn an_absent_file_yields_defaults() {
        let path = std::env::temp_dir().join("rav-arena-config-absent-xyz.toml");
        let _ = std::fs::remove_file(&path);
        let config = ArenaConfig::load(&path).unwrap();
        assert_eq!(config.pairs, 3);
        assert!(config.http.model.is_empty());
    }

    #[test]
    fn keys_override_defaults() {
        let path = write(
            "override",
            "# arena\nmodel = \"vendor/some-model\"\npairs = 12\nbudget = 40\nrecord = true\n",
        );
        let config = ArenaConfig::load(&path).unwrap();
        assert_eq!(config.http.model, "vendor/some-model");
        assert_eq!(config.pairs, 12);
        assert_eq!(config.budget, 40);
        assert!(config.record);
    }

    #[test]
    fn a_typo_is_refused_rather_than_ignored() {
        // A silently-ignored `budjet` would produce a run that cost far more
        // than the operator asked for and looked like it had worked.
        let path = write("typo", "budjet = 10\n");
        let error = ArenaConfig::load(&path).unwrap_err();
        assert!(error.contains("unknown key `budjet`"), "{error}");
    }

    #[test]
    fn a_malformed_number_is_refused() {
        let path = write("malformed", "pairs = many\n");
        assert!(ArenaConfig::load(&path).is_err());
    }
}
