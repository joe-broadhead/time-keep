//! Optional TOML configuration.
//!
//! The config file is optional. When present, it can define a default timezone
//! (or list of timezones) that `now` and the `current_time` MCP tool use when
//! the caller does not pass an explicit timezone. Zero-config behavior is
//! unchanged: with no config and no relevant environment variable, the default
//! remains UTC.
//!
//! The file is only read by the code paths that use the default, so a broken
//! config never blocks unrelated commands. Unknown keys are ignored with a
//! warning so configs written for newer versions keep working on older
//! binaries (and vice versa).

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

/// Tokens that opt in to operating-system timezone detection.
const SYSTEM_TOKENS: [&str; 2] = ["system", "local"];

/// Keys this version of time-keep understands in `config.toml`.
const KNOWN_KEYS: [&str; 2] = ["default_timezone", "default_timezones"];

/// Parsed `config.toml` contents. All fields are optional.
#[derive(Debug, Default, Deserialize)]
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
    /// (empty) config. A present-but-unparseable file is a hard error so
    /// misconfiguration is visible rather than silently ignored, while unknown
    /// keys only warn, keeping configs portable across versions.
    pub(crate) fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::parse(&contents, path),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(TimeKeepError::from(err)
                .with_detail("config_path", serde_json::json!(path.display().to_string()))),
        }
    }

    fn parse(contents: &str, path: &Path) -> Result<Self> {
        let parse_error = |err: toml::de::Error| {
            TimeKeepError::invalid_params(format!(
                "failed to parse config file {}: {err}",
                path.display()
            ))
            .with_detail("config_path", serde_json::json!(path.display().to_string()))
        };
        let table: toml::Table = toml::from_str(contents).map_err(parse_error)?;
        for key in table.keys() {
            if !KNOWN_KEYS.contains(&key.as_str()) {
                tracing::warn!(
                    "ignoring unknown key \"{key}\" in config file {}",
                    path.display()
                );
            }
        }
        table.try_into().map_err(parse_error)
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

/// Resolve the default timezones for `now` / `current_time` from the
/// environment and the config file at `config_path`, applying the precedence:
/// `TIME_KEEP_TZ` environment variable, then config file, then an empty result
/// (which callers treat as UTC).
///
/// The environment is consulted first and short-circuits, so a valid
/// `TIME_KEEP_TZ` works even when the config file is unreadable or invalid.
/// Explicit `--tz` flags and MCP `timezones` arguments are handled by callers
/// and always win over these defaults. The returned names are validated IANA
/// timezones with any `system`/`local` token already expanded.
pub(crate) fn default_timezones_from(config_path: &Path) -> Result<Vec<String>> {
    resolve_from_parts(
        parse_env_value(std::env::var_os(DEFAULT_TZ_ENV))?.as_deref(),
        || Config::load(config_path),
    )
}

/// Pure resolution logic with the environment value and config loader
/// injected, so tests never depend on (or mutate) the live process
/// environment. The loader only runs when the environment yields nothing.
fn resolve_from_parts(
    env_value: Option<&str>,
    load_config: impl FnOnce() -> Result<Config>,
) -> Result<Vec<String>> {
    if let Some(raw) = env_tokens(env_value) {
        return resolve_tokens(&raw, TokenSource::Env);
    }
    let configured = load_config()?.configured_tokens();
    if !configured.is_empty() {
        return resolve_tokens(&configured, TokenSource::Config);
    }
    Ok(Vec::new())
}

/// Convert the raw environment value into a string, rejecting non-UTF-8 bytes
/// explicitly instead of silently treating them as unset.
fn parse_env_value(value: Option<std::ffi::OsString>) -> Result<Option<String>> {
    match value {
        None => Ok(None),
        Some(os_value) => os_value.into_string().map(Some).map_err(|_| {
            TimeKeepError::invalid_params(format!(
                "{DEFAULT_TZ_ENV} contains invalid UTF-8 and cannot be used"
            ))
            .with_detail("source", serde_json::json!(DEFAULT_TZ_ENV))
        }),
    }
}

/// Split a `TIME_KEEP_TZ` value into tokens. Returns `None` when unset or
/// blank (so config can take over).
fn env_tokens(env_value: Option<&str>) -> Option<Vec<String>> {
    let tokens = split_tokens(env_value?);
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
        Config::parse(toml_str, Path::new("/test/config.toml")).expect("valid test config")
    }

    fn no_config() -> Result<Config> {
        Ok(Config::default())
    }

    #[test]
    fn missing_config_file_is_default() {
        let cfg = Config::load(Path::new("/nonexistent/time-keep/config.toml"))
            .expect("missing file is not an error");
        assert!(cfg.default_timezone.is_none());
        assert!(cfg.default_timezones.is_empty());
    }

    #[test]
    fn unknown_keys_are_tolerated() {
        let cfg = config("future_setting = true\ndefault_timezone = \"Europe/Amsterdam\"\n");
        assert_eq!(cfg.default_timezone.as_deref(), Some("Europe/Amsterdam"));
    }

    #[test]
    fn invalid_toml_is_error() {
        let err = Config::parse("not valid toml [", Path::new("/test/config.toml"))
            .expect_err("syntax error should fail");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn wrong_value_type_is_error() {
        let err = Config::parse("default_timezone = 5\n", Path::new("/test/config.toml"))
            .expect_err("type error should fail");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
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
        let resolved = resolve_from_parts(None, || Ok(cfg)).expect("valid configured timezone");
        assert_eq!(resolved, vec!["Europe/Amsterdam".to_string()]);
    }

    #[test]
    fn empty_config_resolves_to_empty() {
        let resolved = resolve_from_parts(None, no_config).expect("empty is ok");
        assert!(resolved.is_empty());
    }

    #[test]
    fn env_overrides_config() {
        let resolved = resolve_from_parts(Some("Asia/Tokyo,UTC"), || {
            Ok(config("default_timezone = \"Europe/Amsterdam\"\n"))
        })
        .expect("valid env timezones");
        assert_eq!(resolved, vec!["Asia/Tokyo".to_string(), "UTC".to_string()]);
    }

    #[test]
    fn env_wins_without_touching_broken_config() {
        // The loader fails hard; a valid env value must short-circuit it.
        let resolved = resolve_from_parts(Some("Asia/Tokyo"), || {
            Err(TimeKeepError::invalid_params("must not be called"))
        })
        .expect("env short-circuits config loading");
        assert_eq!(resolved, vec!["Asia/Tokyo".to_string()]);
    }

    #[test]
    fn blank_env_falls_back_to_config() {
        for blank in ["", "  ", ","] {
            let resolved = resolve_from_parts(Some(blank), || {
                Ok(config("default_timezone = \"Europe/Amsterdam\"\n"))
            })
            .expect("blank env is treated as unset");
            assert_eq!(resolved, vec!["Europe/Amsterdam".to_string()]);
        }
    }

    #[test]
    fn invalid_env_timezone_is_error_not_fallback() {
        let err = resolve_from_parts(Some("Mars/Olympus"), no_config)
            .expect_err("bogus env timezone should fail, not fall back to config");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
        assert_eq!(
            err.details().get("source"),
            Some(&serde_json::json!(DEFAULT_TZ_ENV))
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_env_value_is_explicit_error() {
        use std::os::unix::ffi::OsStringExt;

        let err = parse_env_value(Some(std::ffi::OsString::from_vec(vec![0xFF, 0xFE])))
            .expect_err("non-UTF-8 env must error, not silently fall back");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn valid_env_value_passes_through() {
        let value = parse_env_value(Some(std::ffi::OsString::from("Europe/Amsterdam")))
            .expect("valid UTF-8");
        assert_eq!(value.as_deref(), Some("Europe/Amsterdam"));
        assert_eq!(parse_env_value(None).expect("unset is ok"), None);
    }

    #[test]
    fn invalid_configured_timezone_is_error() {
        let err = resolve_from_parts(None, || Ok(config("default_timezone = \"Mars/Olympus\"\n")))
            .expect_err("bogus timezone should fail");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
        assert_eq!(
            err.details().get("source"),
            Some(&serde_json::json!("config"))
        );
    }

    #[test]
    fn tokens_split_and_trim() {
        assert_eq!(
            split_tokens(" Europe/Amsterdam , Asia/Tokyo ,"),
            vec!["Europe/Amsterdam".to_string(), "Asia/Tokyo".to_string()]
        );
    }
}
