use std::path::Path;

use crate::{
    cli::Cli,
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
}
