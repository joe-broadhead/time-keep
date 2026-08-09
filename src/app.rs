use std::path::Path;

use crate::{
    cli::Cli,
    config,
    db::TimerStore,
    error::Result,
    models::ConfigPaths,
    util::{default_config_path, default_data_dir, timer_db_path},
};

pub(crate) struct App {
    config_path: std::path::PathBuf,
    data_dir: std::path::PathBuf,
}

impl App {
    pub(crate) fn new(cli: &Cli) -> Self {
        let config_path = cli.config.clone().unwrap_or_else(default_config_path);
        let data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);
        Self {
            config_path,
            data_dir,
        }
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

    pub(crate) fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Resolved default timezones for `now` / `current_time`. Loaded lazily so
    /// only the commands that use the default read the environment and config
    /// file: a broken config never blocks unrelated commands such as
    /// `config path` or timers, and a valid `TIME_KEEP_TZ` wins even when the
    /// config file is invalid. Empty means "no configured default", which
    /// callers treat as UTC.
    pub(crate) fn default_timezones(&self) -> Result<Vec<String>> {
        config::default_timezones_from(&self.config_path)
    }

    /// Pick the timezones to report: an explicit request always wins (and
    /// skips config loading entirely); otherwise fall back to the configured
    /// defaults (which may be empty, leaving UTC resolution to the timezone
    /// layer).
    pub(crate) fn now_timezones(&self, requested: &[String]) -> Result<Vec<String>> {
        if requested.is_empty() {
            self.default_timezones()
        } else {
            Ok(requested.to_vec())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    // Tests that exercise config-file and TIME_KEEP_TZ resolution live in
    // tests/scaffold_cli.rs, where the child process environment is fully
    // controlled. Unit tests here stay hermetic: they must not read the live
    // process environment.

    #[test]
    fn now_request_overrides_default_without_reading_config() {
        // The config path is a directory, so any attempt to read it would
        // error: proves an explicit request never touches config or env.
        let app = App {
            config_path: std::env::temp_dir(),
            data_dir: PathBuf::from("/tmp/time-keep"),
        };
        assert_eq!(
            app.now_timezones(&["Asia/Tokyo".to_string()])
                .expect("explicit request skips config"),
            vec!["Asia/Tokyo".to_string()]
        );
    }
}
