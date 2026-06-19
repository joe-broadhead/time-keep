use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::APP_NAME;

pub(crate) fn default_config_path() -> PathBuf {
    xdg_home(
        env::var_os("XDG_CONFIG_HOME"),
        env::var_os("HOME"),
        ".config",
    )
    .join(APP_NAME)
    .join("config.toml")
}

pub(crate) fn default_data_dir() -> PathBuf {
    xdg_home(
        env::var_os("XDG_DATA_HOME"),
        env::var_os("HOME"),
        ".local/share",
    )
    .join(APP_NAME)
}

pub(crate) fn timer_db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("timers.db")
}

fn xdg_home(xdg_value: Option<OsString>, home_value: Option<OsString>, fallback: &str) -> PathBuf {
    absolute_env_path(xdg_value).unwrap_or_else(|| home_dir(home_value).join(fallback))
}

fn home_dir(home_value: Option<OsString>) -> PathBuf {
    non_empty_path(home_value).unwrap_or_else(|| PathBuf::from("."))
}

fn absolute_env_path(value: Option<OsString>) -> Option<PathBuf> {
    let path = non_empty_path(value)?;
    path.is_absolute().then_some(path)
}

fn non_empty_path(value: Option<OsString>) -> Option<PathBuf> {
    let path = PathBuf::from(value?);
    (!path.as_os_str().is_empty()).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xdg_path_prefers_xdg_value() {
        let path = xdg_home(
            Some(OsString::from("/tmp/config")),
            Some(OsString::from("/home/example")),
            ".config",
        );
        assert_eq!(path, PathBuf::from("/tmp/config"));
    }

    #[test]
    fn xdg_path_falls_back_to_home() {
        let path = xdg_home(None, Some(OsString::from("/home/example")), ".local/share");
        assert_eq!(path, PathBuf::from("/home/example/.local/share"));
    }

    #[test]
    fn xdg_path_ignores_empty_or_relative_xdg_values() {
        let empty = xdg_home(
            Some(OsString::from("")),
            Some(OsString::from("/home/example")),
            ".config",
        );
        assert_eq!(empty, PathBuf::from("/home/example/.config"));

        let relative = xdg_home(
            Some(OsString::from("relative")),
            Some(OsString::from("/home/example")),
            ".local/share",
        );
        assert_eq!(relative, PathBuf::from("/home/example/.local/share"));
    }

    #[test]
    fn timer_database_lives_under_data_dir() {
        assert_eq!(
            timer_db_path(Path::new("/tmp/time-keep")),
            PathBuf::from("/tmp/time-keep/timers.db")
        );
    }
}
