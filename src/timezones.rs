// Timezone lookup and conversion behavior is implemented in JOE-168.
use std::str::FromStr;

use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveDateTime, Offset, TimeZone, Timelike, Utc,
};
use chrono_tz::{OffsetComponents, OffsetName, TZ_VARIANTS, Tz};
use serde_json::json;

use crate::{
    cli::TimeFormat,
    error::{Result, TimeKeepError},
    models::{
        TimeResponse, TimeSnapshot, TimezoneConversion, TimezoneInfo, TimezoneList,
        TimezoneTransition,
    },
};

const TRANSITION_SEARCH_DAYS: i64 = 370;
const TRANSITION_SCAN_STEP_HOURS: i64 = 6;

pub(crate) fn current_time(timezones: &[String], format: TimeFormat) -> Result<TimeResponse> {
    current_time_at(Utc::now(), timezones, format)
}

pub(crate) fn current_time_at(
    now_utc: DateTime<Utc>,
    timezones: &[String],
    format: TimeFormat,
) -> Result<TimeResponse> {
    let names = requested_timezones(timezones);
    let mut times = Vec::with_capacity(names.len());
    for name in names {
        let tz = parse_tz(&name)?;
        times.push(snapshot_for_utc(tz, now_utc, format));
    }

    Ok(TimeResponse {
        generated_at_utc: format_utc(now_utc),
        format: format_name(format).to_string(),
        times,
    })
}

/// Validate that `name` is an accepted IANA timezone, returning the same
/// structured error as the time commands on failure. Used when resolving
/// configured/environment default timezones.
pub(crate) fn ensure_valid_timezone(name: &str) -> Result<()> {
    parse_tz(name).map(|_| ())
}

pub(crate) fn list_timezones(region: Option<&str>) -> TimezoneList {
    let normalized_region = region
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_matches('/').to_ascii_lowercase());

    let mut timezones = TZ_VARIANTS
        .iter()
        .map(|timezone| timezone.name())
        .filter(|name| {
            normalized_region.as_ref().is_none_or(|region| {
                name.to_ascii_lowercase().starts_with(&format!("{region}/"))
                    || name.eq_ignore_ascii_case(region)
            })
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    timezones.sort_unstable();

    TimezoneList {
        region: normalized_region,
        count: timezones.len(),
        timezones,
    }
}

pub(crate) fn timezone_info(name: &str) -> Result<TimezoneInfo> {
    timezone_info_at(name, Utc::now())
}

pub(crate) fn timezone_info_at(name: &str, now_utc: DateTime<Utc>) -> Result<TimezoneInfo> {
    let tz = parse_tz(name)?;
    Ok(TimezoneInfo {
        timezone: tz.name().to_string(),
        current: snapshot_for_utc(tz, now_utc, TimeFormat::Rfc3339),
        next_transition: next_transition_after(tz, now_utc),
    })
}

pub(crate) fn convert_timezone(
    datetime: &str,
    from_tz: &str,
    to_tz: &str,
) -> Result<TimezoneConversion> {
    let from = parse_tz(from_tz)?;
    let to = parse_tz(to_tz)?;
    let utc = parse_datetime_in_timezone(datetime, from)?;

    Ok(TimezoneConversion {
        input_datetime: datetime.to_string(),
        from_timezone: from.name().to_string(),
        to_timezone: to.name().to_string(),
        source: snapshot_for_utc(from, utc, TimeFormat::Rfc3339),
        target: snapshot_for_utc(to, utc, TimeFormat::Rfc3339),
    })
}

fn requested_timezones(timezones: &[String]) -> Vec<String> {
    if timezones.is_empty() {
        vec!["UTC".to_string()]
    } else {
        timezones.to_vec()
    }
}

fn parse_tz(name: &str) -> Result<Tz> {
    Tz::from_str(name).map_err(|_| {
        TimeKeepError::invalid_params(format!("invalid IANA timezone name: {name}"))
            .with_detail("parameter", json!("timezone"))
            .with_detail("value", json!(name))
    })
}

fn parse_datetime_in_timezone(input: &str, timezone: Tz) -> Result<DateTime<Utc>> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(input) {
        let utc = datetime.with_timezone(&Utc);
        let input_offset = datetime.offset().local_minus_utc();
        let timezone_offset = offset_seconds(timezone, utc);
        if input_offset != timezone_offset {
            return Err(TimeKeepError::invalid_params(format!(
                "datetime offset does not match source timezone {}: {input}",
                timezone.name()
            ))
            .with_detail("parameter", json!("datetime"))
            .with_detail("value", json!(input))
            .with_detail("timezone", json!(timezone.name()))
            .with_detail("input_offset_seconds", json!(input_offset))
            .with_detail("timezone_offset_seconds", json!(timezone_offset)));
        }
        return Ok(utc);
    }

    for pattern in ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S"] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(input, pattern) {
            return resolve_local_datetime(input, timezone, naive);
        }
    }

    Err(TimeKeepError::invalid_params(format!(
        "datetime must be RFC3339 or a local datetime without offset: {input}"
    ))
    .with_detail("parameter", json!("datetime"))
    .with_detail("value", json!(input)))
}

fn resolve_local_datetime(
    input: &str,
    timezone: Tz,
    naive: NaiveDateTime,
) -> Result<DateTime<Utc>> {
    match timezone.from_local_datetime(&naive) {
        LocalResult::Single(datetime) => Ok(datetime.with_timezone(&Utc)),
        LocalResult::Ambiguous(earliest, latest) => Err(TimeKeepError::invalid_params(format!(
            "ambiguous local datetime for timezone {}: {input}",
            timezone.name()
        ))
        .with_detail("parameter", json!("datetime"))
        .with_detail("value", json!(input))
        .with_detail("timezone", json!(timezone.name()))
        .with_detail(
            "earliest_utc",
            json!(format_utc(earliest.with_timezone(&Utc))),
        )
        .with_detail("latest_utc", json!(format_utc(latest.with_timezone(&Utc))))),
        LocalResult::None => Err(TimeKeepError::invalid_params(format!(
            "nonexistent local datetime for timezone {}: {input}",
            timezone.name()
        ))
        .with_detail("parameter", json!("datetime"))
        .with_detail("value", json!(input))
        .with_detail("timezone", json!(timezone.name()))),
    }
}

fn snapshot_for_utc(timezone: Tz, utc: DateTime<Utc>, format: TimeFormat) -> TimeSnapshot {
    let local = utc.with_timezone(&timezone);
    let offset = local.offset();
    let utc_offset_seconds = offset.fix().local_minus_utc();
    let iso_week = local.iso_week();

    TimeSnapshot {
        timezone: timezone.name().to_string(),
        local_datetime: format_local_datetime(local),
        utc_datetime: format_utc(utc),
        display_datetime: format_datetime(local, utc, format),
        utc_offset: format_offset(utc_offset_seconds),
        utc_offset_seconds,
        is_dst: is_daylight_saving(timezone, utc, offset.dst_offset(), utc_offset_seconds),
        abbreviation: offset.abbreviation().map(str::to_string),
        weekday: local.format("%A").to_string(),
        iso_week: iso_week.week(),
        iso_year: iso_week.year(),
        day_of_year: local.ordinal(),
        unix_epoch: utc.timestamp(),
    }
}

fn is_daylight_saving(
    timezone: Tz,
    utc: DateTime<Utc>,
    dst_offset: Duration,
    utc_offset_seconds: i32,
) -> bool {
    if dst_offset > Duration::zero() {
        return true;
    }
    if dst_offset < Duration::zero() {
        return false;
    }

    let context = dst_context_near(timezone, utc);
    context.has_negative_dst && utc_offset_seconds > context.minimum_utc_offset_seconds
}

struct DstContext {
    has_negative_dst: bool,
    minimum_utc_offset_seconds: i32,
}

fn dst_context_near(timezone: Tz, utc: DateTime<Utc>) -> DstContext {
    let mut sample = utc
        .checked_sub_signed(Duration::days(TRANSITION_SEARCH_DAYS))
        .unwrap_or(utc);
    let end = utc
        .checked_add_signed(Duration::days(TRANSITION_SEARCH_DAYS))
        .unwrap_or(utc);
    let mut has_negative_dst = false;
    let mut minimum_utc_offset_seconds = offset_seconds(timezone, utc);

    while sample <= end {
        let local = sample.with_timezone(&timezone);
        let offset = local.offset();
        has_negative_dst |= offset.dst_offset() < Duration::zero();
        minimum_utc_offset_seconds = minimum_utc_offset_seconds.min(offset.fix().local_minus_utc());

        let Some(next_sample) =
            sample.checked_add_signed(Duration::hours(TRANSITION_SCAN_STEP_HOURS))
        else {
            break;
        };
        sample = next_sample;
    }

    DstContext {
        has_negative_dst,
        minimum_utc_offset_seconds,
    }
}

fn next_transition_after(timezone: Tz, from_utc: DateTime<Utc>) -> Option<TimezoneTransition> {
    let start = truncate_to_second(from_utc);
    let horizon = start + Duration::days(TRANSITION_SEARCH_DAYS);
    let start_offset = offset_seconds(timezone, from_utc);
    let mut low = start;
    let mut high = start + Duration::hours(TRANSITION_SCAN_STEP_HOURS);

    while high <= horizon {
        if offset_seconds(timezone, high) != start_offset {
            let transition = refine_transition(timezone, low, high, start_offset);
            return Some(build_transition(timezone, transition));
        }
        low = high;
        high += Duration::hours(TRANSITION_SCAN_STEP_HOURS);
    }

    None
}

fn truncate_to_second(utc: DateTime<Utc>) -> DateTime<Utc> {
    utc.with_nanosecond(0)
        .expect("zero nanosecond is valid for UTC datetimes")
}

fn refine_transition(
    timezone: Tz,
    mut low: DateTime<Utc>,
    mut high: DateTime<Utc>,
    offset_before: i32,
) -> DateTime<Utc> {
    while high.signed_duration_since(low).num_seconds() > 1 {
        let span_seconds = high.signed_duration_since(low).num_seconds();
        let midpoint = low + Duration::seconds(span_seconds / 2);
        if offset_seconds(timezone, midpoint) == offset_before {
            low = midpoint;
        } else {
            high = midpoint;
        }
    }
    high
}

fn build_transition(timezone: Tz, transition_utc: DateTime<Utc>) -> TimezoneTransition {
    let before_utc = transition_utc - Duration::seconds(1);
    let before = before_utc.with_timezone(&timezone);
    let after = transition_utc.with_timezone(&timezone);
    let before_offset = before.offset().fix().local_minus_utc();
    let after_offset = after.offset().fix().local_minus_utc();

    TimezoneTransition {
        transition_utc: format_utc(transition_utc),
        transition_local: format_local_datetime(after),
        offset_before: format_offset(before_offset),
        offset_before_seconds: before_offset,
        offset_after: format_offset(after_offset),
        offset_after_seconds: after_offset,
        abbreviation_before: before.offset().abbreviation().map(str::to_string),
        abbreviation_after: after.offset().abbreviation().map(str::to_string),
    }
}

fn offset_seconds(timezone: Tz, utc: DateTime<Utc>) -> i32 {
    utc.with_timezone(&timezone)
        .offset()
        .fix()
        .local_minus_utc()
}

fn format_datetime(datetime: DateTime<Tz>, utc: DateTime<Utc>, format: TimeFormat) -> String {
    match format {
        TimeFormat::Rfc3339 | TimeFormat::Iso8601 => format_local_datetime(datetime),
        TimeFormat::Epoch => utc.timestamp().to_string(),
    }
}

fn format_local_datetime(datetime: DateTime<Tz>) -> String {
    let offset_seconds = datetime.offset().fix().local_minus_utc();
    let base = format!(
        "{}{}",
        datetime.format("%Y-%m-%dT%H:%M:%S"),
        fractional_suffix(datetime.nanosecond())
    );
    if offset_seconds == 0 {
        format!("{base}Z")
    } else {
        format!("{base}{}", format_offset(offset_seconds))
    }
}

fn format_utc(datetime: DateTime<Utc>) -> String {
    format!(
        "{}{}Z",
        datetime.format("%Y-%m-%dT%H:%M:%S"),
        fractional_suffix(datetime.nanosecond())
    )
}

fn fractional_suffix(nanosecond: u32) -> String {
    if nanosecond == 0 {
        String::new()
    } else {
        let mut digits = format!("{nanosecond:09}");
        while digits.ends_with('0') {
            digits.pop();
        }
        format!(".{digits}")
    }
}

fn format_name(format: TimeFormat) -> &'static str {
    match format {
        TimeFormat::Rfc3339 => "rfc3339",
        TimeFormat::Iso8601 => "iso8601",
        TimeFormat::Epoch => "epoch",
    }
}

fn format_offset(total_seconds: i32) -> String {
    let sign = if total_seconds < 0 { '-' } else { '+' };
    let absolute = total_seconds.abs();
    let hours = absolute / 3600;
    let minutes = (absolute % 3600) / 60;
    let seconds = absolute % 60;
    if seconds == 0 {
        format!("{sign}{hours:02}:{minutes:02}")
    } else {
        format!("{sign}{hours:02}:{minutes:02}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn utc_datetime(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .expect("valid UTC test datetime")
    }

    #[test]
    fn current_time_defaults_to_utc() {
        let response = current_time_at(utc_datetime(2026, 6, 18, 12), &[], TimeFormat::Rfc3339)
            .expect("current time response");
        assert_eq!(response.times.len(), 1);
        assert_eq!(response.times[0].timezone, "UTC");
        assert_eq!(response.times[0].utc_offset, "+00:00");
        assert!(!response.times[0].is_dst);
    }

    #[test]
    fn invalid_timezone_is_invalid_params() {
        let err = timezone_info_at("Madrid", utc_datetime(2026, 6, 18, 12))
            .expect_err("bare city is not an IANA timezone");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn detects_dst_and_next_transition_for_london() {
        let info =
            timezone_info_at("Europe/London", utc_datetime(2026, 6, 18, 12)).expect("London info");
        assert!(info.current.is_dst);
        assert_eq!(info.current.utc_offset, "+01:00");
        let transition = info
            .next_transition
            .expect("London should have a future transition");
        assert_eq!(transition.offset_before, "+01:00");
        assert_eq!(transition.offset_after, "+00:00");
    }

    #[test]
    fn next_transition_ignores_current_clock_fractional_seconds() {
        let fractional_now = utc_datetime(2026, 6, 18, 12)
            .with_nanosecond(987_654_321)
            .expect("valid nanosecond");
        let info = timezone_info_at("Europe/London", fractional_now).expect("London info");
        let transition = info
            .next_transition
            .expect("London should have a future transition");
        assert_eq!(transition.transition_utc, "2026-10-25T01:00:00Z");
    }

    #[test]
    fn fixed_utc_has_no_next_transition() {
        let info = timezone_info_at("UTC", utc_datetime(2026, 6, 18, 12)).expect("UTC info");
        assert!(info.next_transition.is_none());
        assert!(!info.current.is_dst);
    }

    #[test]
    fn negative_dst_zones_report_daylight_state() {
        let dublin_summer =
            timezone_info_at("Europe/Dublin", utc_datetime(2026, 6, 18, 12)).expect("Dublin info");
        assert_eq!(dublin_summer.current.utc_offset, "+01:00");
        assert!(dublin_summer.current.is_dst);

        let dublin_winter =
            timezone_info_at("Europe/Dublin", utc_datetime(2026, 1, 18, 12)).expect("Dublin info");
        assert_eq!(dublin_winter.current.utc_offset, "+00:00");
        assert!(!dublin_winter.current.is_dst);

        let casablanca_normal =
            timezone_info_at("Africa/Casablanca", utc_datetime(2026, 6, 18, 12))
                .expect("Casablanca info");
        assert_eq!(casablanca_normal.current.utc_offset, "+01:00");
        assert!(casablanca_normal.current.is_dst);

        let casablanca_ramadan =
            timezone_info_at("Africa/Casablanca", utc_datetime(2027, 2, 8, 12))
                .expect("Casablanca info");
        assert_eq!(casablanca_ramadan.current.utc_offset, "+00:00");
        assert!(!casablanca_ramadan.current.is_dst);
    }

    #[test]
    fn convert_rfc3339_between_iana_zones() {
        let converted =
            convert_timezone("2026-06-18T12:00:00Z", "UTC", "Europe/Madrid").expect("conversion");
        assert_eq!(converted.target.local_datetime, "2026-06-18T14:00:00+02:00");
        assert_eq!(converted.target.utc_offset, "+02:00");
    }

    #[test]
    fn historical_sub_minute_offsets_are_preserved() {
        let converted =
            convert_timezone("1900-01-01T00:00:00", "Europe/Paris", "UTC").expect("conversion");
        assert_eq!(
            converted.source.local_datetime,
            "1900-01-01T00:00:00+00:09:21"
        );
        assert_eq!(converted.source.utc_offset, "+00:09:21");
        assert_eq!(converted.source.utc_offset_seconds, 561);
    }

    #[test]
    fn fractional_seconds_are_preserved() {
        let converted = convert_timezone("2026-06-18T12:00:00.500Z", "UTC", "Europe/Madrid")
            .expect("conversion");
        assert_eq!(converted.source.utc_datetime, "2026-06-18T12:00:00.5Z");
        assert_eq!(
            converted.target.local_datetime,
            "2026-06-18T14:00:00.5+02:00"
        );
    }

    #[test]
    fn explicit_datetime_offset_must_match_source_timezone() {
        let err = convert_timezone("2026-06-18T12:00:00+02:00", "UTC", "UTC")
            .expect_err("explicit offset conflicts with UTC source timezone");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn ambiguous_local_datetime_returns_invalid_params() {
        let err = convert_timezone("2026-10-25T01:30:00", "Europe/London", "UTC")
            .expect_err("ambiguous fall-back time");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn nonexistent_local_datetime_returns_invalid_params() {
        // London skips the 01:00 local hour when DST starts on 2026-03-29.
        let err = convert_timezone("2026-03-29T01:30:00", "Europe/London", "UTC")
            .expect_err("spring-forward gap should be rejected");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
        assert_eq!(err.details().get("parameter"), Some(&json!("datetime")));
        assert_eq!(err.details().get("timezone"), Some(&json!("Europe/London")));
    }

    #[test]
    fn list_timezones_filters_by_region() {
        let list = list_timezones(Some("europe"));
        assert!(list.timezones.contains(&"Europe/Madrid".to_string()));
        assert!(!list.timezones.contains(&"America/New_York".to_string()));
    }
}
