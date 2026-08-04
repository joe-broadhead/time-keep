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

/// Best-effort detection of the operating-system IANA timezone.
///
/// This is opt-in: it is only consulted when the user explicitly requests the
/// `system` (or `local`) token via config or `TIME_KEEP_TZ`. It never runs as a
/// silent fallback. Returns `None` when no plausible IANA name can be found, in
/// which case the caller surfaces an explicit error.
pub(crate) fn detect_system_timezone() -> Option<String> {
    system_timezone_from(env::var_os("TZ"), std::fs::read_link("/etc/localtime").ok())
}

/// Pure detection logic, split out for testing.
///
/// Precedence:
/// 1. `TZ` when it is a plausible IANA name (respects container/session overrides).
/// 2. The `.../zoneinfo/<Area>/<Location>` tail of the `/etc/localtime` symlink.
/// 3. `TZ` as a last resort even if it is a bare token like `UTC`.
fn system_timezone_from(
    tz_env: Option<OsString>,
    localtime_link: Option<PathBuf>,
) -> Option<String> {
    let tz_env = tz_env
        .and_then(|value| value.into_string().ok())
        .map(|value| value.trim().trim_start_matches(':').trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(tz) = tz_env.as_deref()
        && looks_like_iana_name(tz)
    {
        return Some(tz.to_string());
    }

    if let Some(name) = zoneinfo_tail(localtime_link.as_deref()) {
        return Some(name);
    }

    tz_env
}

/// Extract the timezone name that follows a `zoneinfo/` path segment.
fn zoneinfo_tail(link: Option<&Path>) -> Option<String> {
    let text = link?.to_str()?;
    let marker = "zoneinfo/";
    let index = text.rfind(marker)?;
    let tail = &text[index + marker.len()..];
    (!tail.is_empty()).then(|| tail.to_string())
}

/// A loose IANA-name check: an `Area/Location` name (optionally with digits or
/// signs, e.g. `Etc/GMT+5`), or a bare all-alphabetic token such as `UTC`/`GMT`.
/// Rejects POSIX TZ rules like `CET-1CEST,M3.5.0,M10.5.0/3`, which always carry
/// a comma or whitespace that IANA names never contain.
fn looks_like_iana_name(value: &str) -> bool {
    if value.is_empty() || value.contains(',') || value.chars().any(char::is_whitespace) {
        return false;
    }
    if value.contains('/') {
        return true;
    }
    value
        .chars()
        .all(|ch| ch.is_ascii_alphabetic() || ch == '_')
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

    #[test]
    fn system_timezone_reads_localtime_symlink() {
        let detected = system_timezone_from(
            None,
            Some(PathBuf::from("/usr/share/zoneinfo/Europe/Amsterdam")),
        );
        assert_eq!(detected.as_deref(), Some("Europe/Amsterdam"));
    }

    #[test]
    fn system_timezone_reads_macos_style_symlink() {
        let detected = system_timezone_from(
            None,
            Some(PathBuf::from("/var/db/timezone/zoneinfo/Asia/Tokyo")),
        );
        assert_eq!(detected.as_deref(), Some("Asia/Tokyo"));
    }

    #[test]
    fn system_timezone_prefers_iana_tz_env() {
        let detected = system_timezone_from(
            Some(OsString::from(":America/New_York")),
            Some(PathBuf::from("/usr/share/zoneinfo/Europe/Amsterdam")),
        );
        assert_eq!(detected.as_deref(), Some("America/New_York"));
    }

    #[test]
    fn system_timezone_ignores_posix_tz_rule_in_favor_of_symlink() {
        let detected = system_timezone_from(
            Some(OsString::from("CET-1CEST,M3.5.0,M10.5.0/3")),
            Some(PathBuf::from("/usr/share/zoneinfo/Europe/Amsterdam")),
        );
        assert_eq!(detected.as_deref(), Some("Europe/Amsterdam"));
    }

    #[test]
    fn system_timezone_accepts_bare_utc_token() {
        let detected = system_timezone_from(Some(OsString::from("UTC")), None);
        assert_eq!(detected.as_deref(), Some("UTC"));
    }

    #[test]
    fn system_timezone_none_when_nothing_detected() {
        assert_eq!(system_timezone_from(None, None), None);
        assert_eq!(
            system_timezone_from(None, Some(PathBuf::from("/etc/localtime"))),
            None
        );
    }
}
