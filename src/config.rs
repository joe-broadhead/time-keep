//! Optional TOML configuration.
//!
//! The config file is optional. When present, it can define a default timezone
//! (or list of timezones) that `now` and the `current_time` MCP tool use when
//! the caller does not pass an explicit timezone. Zero-config behavior is
//! unchanged: with no config and no relevant environment variable, the default
//! remains UTC.

use std::path::Path;

use serde::Deserialize;

use crate::{
    error::{Result, TimeKeepError},
    timezones,
    util::detect_system_timezone,
};

/// Environment variable that overrides the configured default timezone(s).
///
/// Accepts a single IANA name, a comma-separated list, or the special token
/// `system` (alias `local`) to detect the operating-system timezone.
pub(crate) const DEFAULT_TZ_ENV: &str = "TIME_KEEP_TZ";

/// Token that opts in to operating-system timezone detection.
const SYSTEM_TOKENS: [&str; 2] = ["system", "local"];

/// Parsed `config.toml` contents. All fields are optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// Single default timezone. Ignored when `default_timezones` is non-empty.
    #[serde(default)]
    pub(crate) default_timezone: Option<String>,
    /// Ordered list of default timezones. Takes precedence over the singular
    /// `default_timezone` when both are set.
    #[serde(default)]
    pub(crate) default_timezones: Vec<String>,
}

impl Config {
    /// Load a config file if it exists. A missing file yields the default
    /// (empty) config; a present-but-invalid file is a hard error so
    /// misconfiguration is visible rather than silently ignored.
    pub(crate) fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents).map_err(|err| {
                TimeKeepError::invalid_params(format!(
                    "failed to parse config file {}: {err}",
                    path.display()
                ))
                .with_detail("config_path", serde_json::json!(path.display().to_string()))
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(TimeKeepError::from(err)
                .with_detail("config_path", serde_json::json!(path.display().to_string()))),
        }
    }

    /// The raw configured timezone tokens, list taking precedence over the
    /// singular form. Empty when nothing is configured.
    fn configured_tokens(&self) -> Vec<String> {
        if !self.default_timezones.is_empty() {
            self.default_timezones.clone()
        } else if let Some(tz) = &self.default_timezone {
            vec![tz.clone()]
        } else {
            Vec::new()
        }
    }
}

/// Resolve the default timezones for `now` / `current_time`, applying the
/// precedence: `TIME_KEEP_TZ` environment variable, then config file, then an
/// empty result (which callers treat as UTC).
///
/// Explicit `--tz` flags and MCP `timezones` arguments are handled by callers
/// and always win over these defaults. The returned names are validated IANA
/// timezones with any `system`/`local` token already expanded.
pub(crate) fn resolve_default_timezones(config: &Config) -> Result<Vec<String>> {
    if let Some(raw) = env_tokens() {
        return resolve_tokens(&raw, TokenSource::Env);
    }
    let configured = config.configured_tokens();
    if !configured.is_empty() {
        return resolve_tokens(&configured, TokenSource::Config);
    }
    Ok(Vec::new())
}

/// Read and split the `TIME_KEEP_TZ` environment variable into tokens. Returns
/// `None` when unset or empty (so config can take over).
fn env_tokens() -> Option<Vec<String>> {
    let raw = std::env::var(DEFAULT_TZ_ENV).ok()?;
    let tokens = split_tokens(&raw);
    (!tokens.is_empty()).then_some(tokens)
}

fn split_tokens(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(str::to_string)
        .collect()
}

#[derive(Clone, Copy)]
enum TokenSource {
    Env,
    Config,
}

impl TokenSource {
    fn label(self) -> &'static str {
        match self {
            TokenSource::Env => DEFAULT_TZ_ENV,
            TokenSource::Config => "config",
        }
    }
}

/// Expand `system`/`local` tokens and validate every timezone name.
fn resolve_tokens(tokens: &[String], source: TokenSource) -> Result<Vec<String>> {
    let mut resolved = Vec::with_capacity(tokens.len());
    for token in tokens {
        let name = if SYSTEM_TOKENS.contains(&token.to_ascii_lowercase().as_str()) {
            detect_system_timezone().ok_or_else(|| {
                TimeKeepError::invalid_params(format!(
                    "{} requested \"{token}\" but the system timezone could not be detected",
                    source.label()
                ))
                .with_detail("source", serde_json::json!(source.label()))
            })?
        } else {
            token.clone()
        };
        timezones::ensure_valid_timezone(&name)
            .map_err(|err| err.with_detail("source", serde_json::json!(source.label())))?;
        resolved.push(name);
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(toml_str: &str) -> Config {
        toml::from_str(toml_str).expect("valid test config")
    }

    #[test]
    fn missing_config_file_is_default() {
        let cfg = Config::load(Path::new("/nonexistent/time-keep/config.toml"))
            .expect("missing file is not an error");
        assert!(cfg.default_timezone.is_none());
        assert!(cfg.default_timezones.is_empty());
    }

    #[test]
    fn list_takes_precedence_over_singular() {
        let cfg = config(
            "default_timezone = \"UTC\"\ndefault_timezones = [\"Europe/Amsterdam\", \"Asia/Tokyo\"]\n",
        );
        assert_eq!(
            cfg.configured_tokens(),
            vec!["Europe/Amsterdam".to_string(), "Asia/Tokyo".to_string()]
        );
    }

    #[test]
    fn singular_default_is_used_when_no_list() {
        let cfg = config("default_timezone = \"Europe/Amsterdam\"\n");
        let resolved = resolve_tokens(&cfg.configured_tokens(), TokenSource::Config)
            .expect("valid configured timezone");
        assert_eq!(resolved, vec!["Europe/Amsterdam".to_string()]);
    }

    #[test]
    fn empty_config_resolves_to_empty() {
        let resolved = resolve_default_timezones(&Config::default()).expect("empty is ok");
        assert!(resolved.is_empty());
    }

    #[test]
    fn invalid_configured_timezone_is_error() {
        let cfg = config("default_timezone = \"Mars/Olympus\"\n");
        let err = resolve_tokens(&cfg.configured_tokens(), TokenSource::Config)
            .expect_err("bogus timezone should fail");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn unknown_config_key_is_rejected() {
        let err = toml::from_str::<Config>("default_tz = \"UTC\"\n")
            .expect_err("unknown key should fail");
        assert!(err.to_string().contains("default_tz") || err.to_string().contains("unknown"));
    }

    #[test]
    fn tokens_split_and_trim() {
        assert_eq!(
            split_tokens(" Europe/Amsterdam , Asia/Tokyo ,"),
            vec!["Europe/Amsterdam".to_string(), "Asia/Tokyo".to_string()]
        );
    }
}
