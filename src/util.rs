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

/// Result of resolving the effective operating-system timezone to an IANA
/// identifier.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SystemTimezoneDetection {
    Detected(String),
    Unavailable,
    /// `TZ` is an explicit process-level override. If it cannot be represented
    /// by the IANA-only public contract, using the machine timezone instead
    /// would silently report the wrong wall-clock time.
    InvalidTzOverride(String),
    InvalidSystemTimezone(String),
}

/// Detect the effective IANA timezone when the user explicitly opts in with the
/// `system`/`local` token.
///
/// An explicit `TZ` always has precedence. IANA names and zoneinfo paths are
/// accepted; valid POSIX rule strings cannot be represented by time-keep's
/// IANA-only output contract and therefore produce an explicit error rather
/// than silently falling back to another timezone. When `TZ` is unset, the
/// cross-platform detector supports Linux, macOS, Windows, and other targets.
pub(crate) fn detect_system_timezone() -> SystemTimezoneDetection {
    system_timezone_from(env::var_os("TZ"), operating_system_timezone)
}

/// Retain generic `/etc/localtime` symlink extraction for layouts outside the
/// platform detector's fixed path prefixes. On Linux and Hurd the link is the
/// effective system setting, so it must beat a potentially stale fallback such
/// as `/etc/timezone`; native platform APIs stay authoritative elsewhere.
fn operating_system_timezone() -> Option<String> {
    let localtime_path = Path::new("/etc/localtime");
    operating_system_timezone_from(
        iana_time_zone::get_timezone().ok(),
        zoneinfo_name_from_path(localtime_path),
        cfg!(any(target_os = "linux", target_os = "hurd")),
        std::fs::symlink_metadata(localtime_path).is_ok(),
        timezone_file_matches_localtime,
    )
}

fn operating_system_timezone_from(
    detected: Option<String>,
    localtime: Option<String>,
    prefer_localtime: bool,
    localtime_entry_exists: bool,
    timezone_matches_localtime: impl FnOnce(&str) -> bool,
) -> Option<String> {
    if prefer_localtime && localtime.is_some() {
        return localtime;
    }
    if let Some(name) = detected.as_deref().and_then(normalized_timezone_name) {
        // A copied, bind-mounted, or custom-linked tzfile is the effective
        // Linux/Hurd setting. Do not trust a potentially stale /etc/timezone
        // fallback unless its named zone has identical tzfile data.
        if prefer_localtime && localtime_entry_exists && !timezone_matches_localtime(&name) {
            return None;
        }
        return Some(name);
    }
    localtime.or(detected)
}

fn timezone_file_matches_localtime(name: &str) -> bool {
    let Ok(localtime) = std::fs::read("/etc/localtime") else {
        return false;
    };
    for root in ["/usr/share/zoneinfo", "/etc/zoneinfo"] {
        for variant in [None, Some("posix"), Some("right")] {
            let root = Path::new(root);
            let path =
                variant.map_or_else(|| root.join(name), |variant| root.join(variant).join(name));
            if std::fs::read(path).is_ok_and(|candidate| candidate == localtime) {
                return true;
            }
        }
    }
    false
}

/// Pure detection logic with the OS lookup injected for deterministic tests.
fn system_timezone_from(
    tz_env: Option<OsString>,
    detect_os_timezone: impl FnOnce() -> Option<String>,
) -> SystemTimezoneDetection {
    if let Some(value) = tz_env {
        let Ok(value) = value.into_string() else {
            return SystemTimezoneDetection::InvalidTzOverride("<non-UTF-8>".to_string());
        };
        let value = value.trim();
        if value.is_empty() {
            // POSIX specifies an empty TZ value as Coordinated Universal Time.
            return SystemTimezoneDetection::Detected("UTC".to_string());
        }
        let tz = value.trim_start_matches(':').trim();
        if tz.is_empty() {
            return SystemTimezoneDetection::Detected("UTC".to_string());
        }
        if let Some(name) = normalized_timezone_name(tz) {
            return SystemTimezoneDetection::Detected(name);
        }
        if tz.starts_with('/')
            && let Some(name) = zoneinfo_name_from_path(Path::new(tz))
        {
            return SystemTimezoneDetection::Detected(name);
        }
        return SystemTimezoneDetection::InvalidTzOverride(value.to_string());
    }

    match detect_os_timezone() {
        Some(name) => normalized_timezone_name(&name)
            .map(SystemTimezoneDetection::Detected)
            .unwrap_or(SystemTimezoneDetection::InvalidSystemTimezone(name)),
        None => SystemTimezoneDetection::Unavailable,
    }
}

/// Resolve an explicit zoneinfo path, following a symlink such as
/// `/etc/localtime` when the literal path does not expose the IANA name.
fn zoneinfo_name_from_path(path: &Path) -> Option<String> {
    zoneinfo_name_from_path_with(path, |path| std::fs::canonicalize(path).ok())
}

fn zoneinfo_name_from_path_with(
    path: &Path,
    canonicalize: impl FnOnce(&Path) -> Option<PathBuf>,
) -> Option<String> {
    zoneinfo_tail(Some(path)).or_else(|| {
        let canonical = canonicalize(path)?;
        zoneinfo_tail(Some(&canonical))
    })
}

/// Extract and validate the timezone name that follows a `zoneinfo/` path
/// segment, normalizing the `posix/` and `right/` tzdata variant directories.
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

/// Normalize tzdata's alternate trees to the public IANA identifier.
fn normalized_timezone_name(value: &str) -> Option<String> {
    let mut name = value.trim();
    for variant in ["posix/", "right/"] {
        if let Some(stripped) = name.strip_prefix(variant) {
            name = stripped;
            break;
        }
    }
    is_valid_timezone_name(name).then(|| name.to_string())
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
    fn system_timezone_uses_cross_platform_os_detection() {
        let detected = system_timezone_from(None, || Some("Europe/Amsterdam".to_string()));
        assert_eq!(
            detected,
            SystemTimezoneDetection::Detected("Europe/Amsterdam".to_string())
        );
    }

    #[test]
    fn os_timezone_falls_back_to_resolved_localtime() {
        for detected in [None, Some("unmappable OS value".to_string())] {
            assert_eq!(
                operating_system_timezone_from(
                    detected,
                    Some("Europe/Paris".to_string()),
                    false,
                    false,
                    |_| false,
                )
                .as_deref(),
                Some("Europe/Paris")
            );
        }
    }

    #[test]
    fn os_timezone_prefers_valid_platform_result_over_symlink_fallback() {
        assert_eq!(
            operating_system_timezone_from(
                Some("Asia/Tokyo".to_string()),
                Some("Europe/Paris".to_string()),
                false,
                false,
                |_| false,
            )
            .as_deref(),
            Some("Asia/Tokyo")
        );
    }

    #[test]
    fn os_timezone_prefers_effective_linux_localtime_over_stale_fallback() {
        assert_eq!(
            operating_system_timezone_from(
                Some("Etc/UTC".to_string()),
                Some("Europe/Paris".to_string()),
                true,
                true,
                |_| panic!("resolved localtime must skip fallback verification"),
            )
            .as_deref(),
            Some("Europe/Paris")
        );
    }

    #[test]
    fn os_timezone_verifies_unresolved_linux_localtime_before_trusting_fallback() {
        for (matches, expected) in [(true, Some("Etc/UTC")), (false, None)] {
            assert_eq!(
                operating_system_timezone_from(
                    Some("Etc/UTC".to_string()),
                    None,
                    true,
                    true,
                    |_| matches,
                )
                .as_deref(),
                expected
            );
        }
    }

    #[test]
    fn system_timezone_normalizes_os_tzdata_variant_names() {
        for (name, expected) in [
            ("posix/Europe/Amsterdam", "Europe/Amsterdam"),
            ("right/Asia/Tokyo", "Asia/Tokyo"),
            ("  America/New_York  ", "America/New_York"),
        ] {
            assert_eq!(
                system_timezone_from(None, || Some(name.to_string())),
                SystemTimezoneDetection::Detected(expected.to_string())
            );
        }
    }

    #[test]
    fn system_timezone_prefers_iana_tz_env() {
        let detected = system_timezone_from(Some(OsString::from(":America/New_York")), || {
            panic!("explicit TZ must skip OS detection")
        });
        assert_eq!(
            detected,
            SystemTimezoneDetection::Detected("America/New_York".to_string())
        );
    }

    #[test]
    fn system_timezone_accepts_digit_bearing_iana_tz_env() {
        // EST5EDT is a valid IANA name; it must beat OS detection.
        let detected = system_timezone_from(Some(OsString::from("EST5EDT")), || {
            Some("Europe/Amsterdam".to_string())
        });
        assert_eq!(
            detected,
            SystemTimezoneDetection::Detected("EST5EDT".to_string())
        );
    }

    #[test]
    fn system_timezone_resolves_posix_path_form_tz_env() {
        // POSIX allows TZ to point at a zoneinfo file directly.
        let detected = system_timezone_from(
            Some(OsString::from(":/usr/share/zoneinfo/Europe/Paris")),
            || Some("Asia/Tokyo".to_string()),
        );
        assert_eq!(
            detected,
            SystemTimezoneDetection::Detected("Europe/Paris".to_string())
        );
    }

    #[test]
    fn zoneinfo_path_resolution_follows_localtime_symlink() {
        let resolved = zoneinfo_name_from_path_with(Path::new("/etc/localtime"), |path| {
            assert_eq!(path, Path::new("/etc/localtime"));
            Some(PathBuf::from("/usr/share/zoneinfo/posix/Europe/Amsterdam"))
        });
        assert_eq!(resolved.as_deref(), Some("Europe/Amsterdam"));
    }

    #[test]
    fn system_timezone_normalizes_posix_and_right_zoneinfo_paths() {
        for (path, expected) in [
            (
                ":/usr/share/zoneinfo/posix/Europe/Amsterdam",
                "Europe/Amsterdam",
            ),
            (":/usr/share/zoneinfo/right/Asia/Tokyo", "Asia/Tokyo"),
        ] {
            let detected = system_timezone_from(Some(OsString::from(path)), || None);
            assert_eq!(
                detected,
                SystemTimezoneDetection::Detected(expected.to_string())
            );
        }
    }

    #[test]
    fn system_timezone_rejects_unmappable_posix_rule_instead_of_falling_back() {
        let rule = "CET-1CEST,M3.5.0,M10.5.0/3";
        let detected = system_timezone_from(Some(OsString::from(rule)), || {
            Some("Europe/Amsterdam".to_string())
        });
        assert_eq!(
            detected,
            SystemTimezoneDetection::InvalidTzOverride(rule.to_string())
        );
    }

    #[test]
    fn system_timezone_accepts_bare_utc_token() {
        let detected = system_timezone_from(Some(OsString::from("UTC")), || None);
        assert_eq!(
            detected,
            SystemTimezoneDetection::Detected("UTC".to_string())
        );
    }

    #[test]
    fn empty_tz_override_means_utc_instead_of_falling_back() {
        for value in ["", "   ", ":", "::"] {
            assert_eq!(
                system_timezone_from(Some(OsString::from(value)), || {
                    Some("Europe/Amsterdam".to_string())
                }),
                SystemTimezoneDetection::Detected("UTC".to_string())
            );
        }
    }

    #[test]
    fn invalid_tz_override_is_not_replaced_by_os_timezone() {
        assert_eq!(
            system_timezone_from(Some(OsString::from("Not/A_Zone")), || {
                Some("Europe/Amsterdam".to_string())
            }),
            SystemTimezoneDetection::InvalidTzOverride("Not/A_Zone".to_string())
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_tz_override_is_reported_explicitly() {
        use std::os::unix::ffi::OsStringExt;

        assert_eq!(
            system_timezone_from(Some(OsString::from_vec(vec![0xFF, 0xFE])), || None),
            SystemTimezoneDetection::InvalidTzOverride("<non-UTF-8>".to_string())
        );
    }

    #[test]
    fn invalid_os_timezone_is_reported_explicitly() {
        assert_eq!(
            system_timezone_from(None, || Some("Not/A_Zone".to_string())),
            SystemTimezoneDetection::InvalidSystemTimezone("Not/A_Zone".to_string())
        );
    }

    #[test]
    fn system_timezone_is_unavailable_when_os_detection_fails() {
        assert_eq!(
            system_timezone_from(None, || None),
            SystemTimezoneDetection::Unavailable
        );
    }
}
