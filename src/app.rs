use std::path::Path;

use crate::{
    cli::Cli,
    config::{Config, resolve_default_timezones},
    db::TimerStore,
    error::Result,
    models::ConfigPaths,
    util::{default_config_path, default_data_dir, timer_db_path},
};

pub(crate) struct App {
    config_path: std::path::PathBuf,
    data_dir: std::path::PathBuf,
    default_timezones: Vec<String>,
}

impl App {
    pub(crate) fn new(cli: &Cli) -> Result<Self> {
        let config_path = cli.config.clone().unwrap_or_else(default_config_path);
        let data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);
        let config = Config::load(&config_path)?;
        let default_timezones = resolve_default_timezones(&config)?;
        Ok(Self {
            config_path,
            data_dir,
            default_timezones,
        })
    }

    pub(crate) fn config_paths(&self) -> ConfigPaths {
        ConfigPaths {
            config_path: self.config_path.display().to_string(),
            data_dir: self.data_dir.display().to_string(),
            timer_db_path: timer_db_path(&self.data_dir).display().to_string(),
        }
    }

    pub(crate) fn timer_store(&self) -> Result<TimerStore> {
        TimerStore::open(timer_db_path(&self.data_dir))
    }

    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Resolved default timezones for `now` / `current_time`. Empty means "no
    /// configured default", which callers treat as UTC.
    pub(crate) fn default_timezones(&self) -> &[String] {
        &self.default_timezones
    }

    /// Pick the timezones to report: an explicit request always wins; otherwise
    /// fall back to the configured defaults (which may be empty, leaving UTC
    /// resolution to the timezone layer).
    pub(crate) fn now_timezones(&self, requested: &[String]) -> Vec<String> {
        if requested.is_empty() {
            self.default_timezones.clone()
        } else {
            requested.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_defaults(defaults: Vec<String>) -> App {
        App {
            config_path: std::path::PathBuf::from("/tmp/time-keep/config.toml"),
            data_dir: std::path::PathBuf::from("/tmp/time-keep"),
            default_timezones: defaults,
        }
    }

    #[test]
    fn now_uses_configured_default_when_no_request() {
        let app = app_with_defaults(vec!["Europe/Amsterdam".to_string()]);
        assert_eq!(app.now_timezones(&[]), vec!["Europe/Amsterdam".to_string()]);
    }

    #[test]
    fn now_request_overrides_configured_default() {
        let app = app_with_defaults(vec!["Europe/Amsterdam".to_string()]);
        assert_eq!(
            app.now_timezones(&["Asia/Tokyo".to_string()]),
            vec!["Asia/Tokyo".to_string()]
        );
    }

    #[test]
    fn now_is_empty_without_configuration_leaving_utc_downstream() {
        let app = app_with_defaults(Vec::new());
        assert!(app.now_timezones(&[]).is_empty());
    }
}
