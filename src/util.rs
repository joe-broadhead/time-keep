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

/// Pure detection logic, split out for testing. Every result is validated
/// against the real IANA database, never guessed from shape.
///
/// Precedence:
/// 1. `TZ` when it parses as an IANA name (respects container/session
///    overrides, including digit-bearing names like `EST5EDT`).
/// 2. `TZ` in the POSIX zoneinfo-path form (`TZ=:/usr/share/zoneinfo/<name>`),
///    resolved through the same path extraction as the symlink.
/// 3. The `.../zoneinfo/<Area>/<Location>` tail of the `/etc/localtime`
///    symlink, with `posix/` and `right/` tzdata variants normalized away.
///
/// POSIX rule strings such as `CET-1CEST,M3.5.0,M10.5.0/3` are not IANA names
/// and fall through to the symlink.
fn system_timezone_from(
    tz_env: Option<OsString>,
    localtime_link: Option<PathBuf>,
) -> Option<String> {
    let tz_env = tz_env
        .and_then(|value| value.into_string().ok())
        .map(|value| value.trim().trim_start_matches(':').trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(tz) = tz_env.as_deref() {
        if is_valid_timezone_name(tz) {
            return Some(tz.to_string());
        }
        if tz.starts_with('/')
            && let Some(name) = zoneinfo_tail(Some(Path::new(tz)))
        {
            return Some(name);
        }
    }

    zoneinfo_tail(localtime_link.as_deref())
}

/// Extract and validate the timezone name that follows a `zoneinfo/` path
/// segment, normalizing the `posix/` and `right/` tzdata variant directories
/// some distributions link through.
fn zoneinfo_tail(link: Option<&Path>) -> Option<String> {
    let text = link?.to_str()?;
    let marker = "zoneinfo/";
    let index = text.rfind(marker)?;
    let mut tail = &text[index + marker.len()..];
    for variant in ["posix/", "right/"] {
        if let Some(stripped) = tail.strip_prefix(variant) {
            tail = stripped;
        }
    }
    (is_valid_timezone_name(tail)).then(|| tail.to_string())
}

/// Whether `value` is a real IANA timezone name, checked against the embedded
/// database rather than inferred from its shape.
fn is_valid_timezone_name(value: &str) -> bool {
    crate::timezones::ensure_valid_timezone(value).is_ok()
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
    fn system_timezone_strips_posix_and_right_tzdata_variants() {
        let posix = system_timezone_from(
            None,
            Some(PathBuf::from("/usr/share/zoneinfo/posix/Europe/Amsterdam")),
        );
        assert_eq!(posix.as_deref(), Some("Europe/Amsterdam"));

        let right = system_timezone_from(
            None,
            Some(PathBuf::from("/usr/share/zoneinfo/right/Asia/Tokyo")),
        );
        assert_eq!(right.as_deref(), Some("Asia/Tokyo"));
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
    fn system_timezone_accepts_digit_bearing_iana_tz_env() {
        // EST5EDT is a valid IANA name; it must beat the symlink.
        let detected = system_timezone_from(
            Some(OsString::from("EST5EDT")),
            Some(PathBuf::from("/usr/share/zoneinfo/Europe/Amsterdam")),
        );
        assert_eq!(detected.as_deref(), Some("EST5EDT"));
    }

    #[test]
    fn system_timezone_resolves_posix_path_form_tz_env() {
        // POSIX allows TZ to point at a zoneinfo file directly.
        let detected = system_timezone_from(
            Some(OsString::from(":/usr/share/zoneinfo/Europe/Paris")),
            Some(PathBuf::from("/usr/share/zoneinfo/Asia/Tokyo")),
        );
        assert_eq!(detected.as_deref(), Some("Europe/Paris"));
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
        // Garbage TZ with no symlink must not be returned verbatim.
        assert_eq!(
            system_timezone_from(Some(OsString::from("Not/A_Zone")), None),
            None
        );
    }
}
