use chrono::{
    DateTime, Datelike, Duration, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Timelike,
    Weekday, format::strftime::StrftimeItems,
};
use serde_json::json;

use crate::{
    cli::{DateOutputFormat, DateUnit},
    error::{Result, TimeKeepError},
    models::{CalendarQuery, DateArithmetic, DateDiff, DateFormatResult},
};

const DATE_PATTERN: &str = "%Y-%m-%d";
const DATETIME_T_PATTERN: &str = "%Y-%m-%dT%H:%M:%S";
const DATETIME_T_FRACTIONAL_PATTERN: &str = "%Y-%m-%dT%H:%M:%S%.f";
const DATETIME_T_OFFSET_PATTERN: &str = "%Y-%m-%dT%H:%M:%S%:z";
const DATETIME_T_FRACTIONAL_OFFSET_PATTERN: &str = "%Y-%m-%dT%H:%M:%S%.f%:z";
const DATETIME_SPACE_PATTERN: &str = "%Y-%m-%d %H:%M:%S";
const DATETIME_SPACE_FRACTIONAL_PATTERN: &str = "%Y-%m-%d %H:%M:%S%.f";

pub(crate) fn calendar_query(input: &str) -> Result<CalendarQuery> {
    let date = parse_date(input)?;
    let iso_week = date.iso_week();

    Ok(CalendarQuery {
        date: format_date(date),
        weekday: weekday_name(date.weekday()).to_string(),
        weekday_number_from_monday: date.weekday().number_from_monday(),
        iso_week: iso_week.week(),
        iso_year: iso_week.year(),
        day_of_year: date.ordinal(),
        days_in_month: days_in_month(date.year(), date.month())?,
        leap_year: is_leap_year(date.year()),
        quarter: ((date.month() - 1) / 3) + 1,
    })
}

pub(crate) fn add(input: &str, amount: i64, unit: &str) -> Result<DateArithmetic> {
    let unit = parse_date_unit(unit)?;
    arithmetic(input, "add", amount, amount, unit)
}

pub(crate) fn subtract(input: &str, amount: i64, unit: &str) -> Result<DateArithmetic> {
    let unit = parse_date_unit(unit)?;
    let signed_amount = amount.checked_neg().ok_or_else(arithmetic_overflow_error)?;
    arithmetic(input, "subtract", amount, signed_amount, unit)
}

pub(crate) fn diff(from: &str, to: &str) -> Result<DateDiff> {
    let from_temporal = parse_temporal(from)?;
    let to_temporal = parse_temporal(to)?;
    let from_datetime = from_temporal.epoch_like_datetime();
    let to_datetime = to_temporal.epoch_like_datetime();
    let duration = to_datetime.signed_duration_since(from_datetime);
    let seconds = duration.num_seconds();

    Ok(DateDiff {
        from: from_temporal.format_normalized(),
        to: to_temporal.format_normalized(),
        from_kind: from_temporal.kind_name().to_string(),
        to_kind: to_temporal.kind_name().to_string(),
        signed_seconds: seconds,
        signed_minutes: seconds / 60,
        signed_hours: seconds / 3_600,
        signed_days: seconds / 86_400,
        signed_weeks: seconds / 604_800,
        direction: temporal_ordering(from_datetime, to_datetime),
        absolute_seconds: seconds.unsigned_abs(),
    })
}

pub(crate) fn format_datetime(
    input: &str,
    output_format: DateOutputFormat,
    strftime: Option<&str>,
    input_format: Option<&str>,
) -> Result<DateFormatResult> {
    let temporal = parse_temporal_with_hint(input, input_format)?;
    let formatted = match output_format {
        DateOutputFormat::Iso8601 => temporal.format_iso8601_with_default_utc(),
        DateOutputFormat::Rfc3339 => temporal.format_rfc3339_with_default_utc()?,
        DateOutputFormat::Rfc2822 => temporal.format_rfc2822_with_default_utc()?,
        DateOutputFormat::Epoch | DateOutputFormat::UnixTimestamp => {
            temporal.format_epoch_seconds_with_default_utc()
        }
        DateOutputFormat::Strftime => {
            let pattern = strftime.ok_or_else(|| {
                TimeKeepError::invalid_params(
                    "--strftime is required when --output-format is strftime",
                )
                .with_detail("parameter", json!("strftime"))
            })?;
            temporal.format_strftime_with_default_utc(pattern)?
        }
    };

    Ok(DateFormatResult {
        input: input.to_string(),
        input_kind: temporal.kind_name().to_string(),
        output_format: date_output_format_name(output_format).to_string(),
        formatted,
        timezone_present: temporal.has_timezone(),
    })
}

fn arithmetic(
    input: &str,
    operation: &'static str,
    amount: i64,
    signed_amount: i64,
    unit: DateUnit,
) -> Result<DateArithmetic> {
    let temporal = parse_temporal(input)?;
    let result = temporal.apply(signed_amount, unit)?;

    Ok(DateArithmetic {
        input: input.to_string(),
        input_kind: temporal.kind_name().to_string(),
        operation: operation.to_string(),
        amount,
        unit: date_unit_name(unit).to_string(),
        result: result.format_normalized(),
        result_kind: result.kind_name().to_string(),
        month_end_clamped: result.month_end_clamped(),
    })
}

#[derive(Debug, Clone)]
enum Temporal {
    Date {
        date: NaiveDate,
        clamped: bool,
    },
    NaiveDateTime {
        datetime: NaiveDateTime,
        clamped: bool,
    },
    OffsetDateTime {
        datetime: DateTime<FixedOffset>,
        clamped: bool,
    },
}

impl Temporal {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Date { .. } => "date",
            Self::NaiveDateTime { .. } => "naive_datetime",
            Self::OffsetDateTime { .. } => "offset_datetime",
        }
    }

    fn has_timezone(&self) -> bool {
        matches!(self, Self::OffsetDateTime { .. })
    }

    fn month_end_clamped(&self) -> bool {
        match self {
            Self::Date { clamped, .. }
            | Self::NaiveDateTime { clamped, .. }
            | Self::OffsetDateTime { clamped, .. } => *clamped,
        }
    }

    fn apply(&self, amount: i64, unit: DateUnit) -> Result<Self> {
        match self {
            Self::Date { date, .. } => apply_to_date(*date, amount, unit),
            Self::NaiveDateTime { datetime, .. } => {
                apply_to_naive_datetime(*datetime, amount, unit)
            }
            Self::OffsetDateTime { datetime, .. } => {
                apply_to_offset_datetime(*datetime, amount, unit)
            }
        }
    }

    fn format_normalized(&self) -> String {
        match self {
            Self::Date { date, .. } => format_date(*date),
            Self::NaiveDateTime { datetime, .. } => format_naive_datetime(*datetime),
            Self::OffsetDateTime { datetime, .. } => format_offset_datetime(*datetime),
        }
    }

    fn epoch_like_datetime(&self) -> NaiveDateTime {
        match self {
            Self::Date { date, .. } => date.and_hms_opt(0, 0, 0).expect("midnight is valid"),
            Self::NaiveDateTime { datetime, .. } => *datetime,
            Self::OffsetDateTime { datetime, .. } => datetime.naive_utc(),
        }
    }

    fn format_iso8601_with_default_utc(&self) -> String {
        format_offset_datetime(self.to_default_utc_datetime())
    }

    fn format_rfc3339_with_default_utc(&self) -> Result<String> {
        let datetime = self.to_default_utc_datetime();
        let year = datetime.year();
        if !(0..=9999).contains(&year) {
            return Err(rfc3339_year_error(year));
        }
        Ok(format_offset_datetime(datetime))
    }

    fn format_rfc2822_with_default_utc(&self) -> Result<String> {
        let datetime = self.to_default_utc_datetime();
        let year = datetime.year();
        if !(0..=9999).contains(&year) {
            return Err(rfc2822_year_error(year));
        }
        Ok(datetime.to_rfc2822())
    }

    fn format_epoch_seconds_with_default_utc(&self) -> String {
        self.to_default_utc_datetime().timestamp().to_string()
    }

    fn format_strftime_with_default_utc(&self, pattern: &str) -> Result<String> {
        let items = StrftimeItems::new(pattern)
            .parse_to_owned()
            .map_err(|_| invalid_strftime_error(pattern))?;
        let mut formatted = String::new();
        self.to_default_utc_datetime()
            .format_with_items(items.iter())
            .write_to(&mut formatted)
            .map_err(|_| invalid_strftime_error(pattern))?;
        Ok(formatted)
    }

    fn to_default_utc_datetime(&self) -> DateTime<FixedOffset> {
        match self {
            Self::Date { date, .. } => DateTime::from_naive_utc_and_offset(
                date.and_hms_opt(0, 0, 0).expect("midnight is valid"),
                utc_fixed_offset(),
            ),
            Self::NaiveDateTime { datetime, .. } => {
                DateTime::from_naive_utc_and_offset(*datetime, utc_fixed_offset())
            }
            Self::OffsetDateTime { datetime, .. } => *datetime,
        }
    }
}

fn apply_to_date(date: NaiveDate, amount: i64, unit: DateUnit) -> Result<Temporal> {
    match unit {
        DateUnit::Seconds | DateUnit::Minutes | DateUnit::Hours => {
            let datetime = date.and_hms_opt(0, 0, 0).expect("midnight is valid");
            apply_to_naive_datetime(datetime, amount, unit)
        }
        DateUnit::Days | DateUnit::Weeks => Ok(Temporal::Date {
            date: shift_date_by_duration(date, amount, unit)?,
            clamped: false,
        }),
        DateUnit::Months | DateUnit::Years => {
            let (date, clamped) = shift_date_by_months(date, amount, unit)?;
            Ok(Temporal::Date { date, clamped })
        }
    }
}

fn apply_to_naive_datetime(
    datetime: NaiveDateTime,
    amount: i64,
    unit: DateUnit,
) -> Result<Temporal> {
    match unit {
        DateUnit::Seconds
        | DateUnit::Minutes
        | DateUnit::Hours
        | DateUnit::Days
        | DateUnit::Weeks => Ok(Temporal::NaiveDateTime {
            datetime: shift_naive_datetime_by_duration(datetime, amount, unit)?,
            clamped: false,
        }),
        DateUnit::Months | DateUnit::Years => {
            let (date, clamped) = shift_date_by_months(datetime.date(), amount, unit)?;
            Ok(Temporal::NaiveDateTime {
                datetime: date
                    .and_hms_nano_opt(
                        datetime.hour(),
                        datetime.minute(),
                        datetime.second(),
                        datetime.nanosecond(),
                    )
                    .expect("existing time remains valid"),
                clamped,
            })
        }
    }
}

fn apply_to_offset_datetime(
    datetime: DateTime<FixedOffset>,
    amount: i64,
    unit: DateUnit,
) -> Result<Temporal> {
    match unit {
        DateUnit::Seconds
        | DateUnit::Minutes
        | DateUnit::Hours
        | DateUnit::Days
        | DateUnit::Weeks => Ok(Temporal::OffsetDateTime {
            datetime: shift_offset_datetime_by_duration(datetime, amount, unit)?,
            clamped: false,
        }),
        DateUnit::Months | DateUnit::Years => {
            let naive = datetime.naive_local();
            let (date, clamped) = shift_date_by_months(naive.date(), amount, unit)?;
            let shifted_naive = date
                .and_hms_nano_opt(
                    naive.hour(),
                    naive.minute(),
                    naive.second(),
                    naive.nanosecond(),
                )
                .expect("existing time remains valid");
            let offset = *datetime.offset();
            let datetime = offset
                .from_local_datetime(&shifted_naive)
                .single()
                .ok_or_else(arithmetic_overflow_error)?;
            Ok(Temporal::OffsetDateTime {
                datetime: ensure_offset_datetime_local_in_range(datetime)?,
                clamped,
            })
        }
    }
}

fn shift_date_by_duration(date: NaiveDate, amount: i64, unit: DateUnit) -> Result<NaiveDate> {
    let duration = match unit {
        DateUnit::Days => Duration::try_days(amount),
        DateUnit::Weeks => amount.checked_mul(7).and_then(Duration::try_days),
        _ => unreachable!("date duration only supports days and weeks"),
    }
    .ok_or_else(arithmetic_overflow_error)?;

    date.checked_add_signed(duration)
        .ok_or_else(arithmetic_overflow_error)
}

fn shift_naive_datetime_by_duration(
    datetime: NaiveDateTime,
    amount: i64,
    unit: DateUnit,
) -> Result<NaiveDateTime> {
    datetime
        .checked_add_signed(duration_for(amount, unit)?)
        .ok_or_else(arithmetic_overflow_error)
}

fn shift_offset_datetime_by_duration(
    datetime: DateTime<FixedOffset>,
    amount: i64,
    unit: DateUnit,
) -> Result<DateTime<FixedOffset>> {
    let shifted = datetime
        .checked_add_signed(duration_for(amount, unit)?)
        .ok_or_else(arithmetic_overflow_error)?;
    ensure_offset_datetime_local_in_range(shifted)
}

fn ensure_offset_datetime_local_in_range(
    datetime: DateTime<FixedOffset>,
) -> Result<DateTime<FixedOffset>> {
    datetime
        .naive_utc()
        .checked_add_offset(*datetime.offset())
        .ok_or_else(arithmetic_overflow_error)?;
    Ok(datetime)
}

fn duration_for(amount: i64, unit: DateUnit) -> Result<Duration> {
    let duration = match unit {
        DateUnit::Seconds => Duration::try_seconds(amount),
        DateUnit::Minutes => Duration::try_minutes(amount),
        DateUnit::Hours => Duration::try_hours(amount),
        DateUnit::Days => Duration::try_days(amount),
        DateUnit::Weeks => amount.checked_mul(7).and_then(Duration::try_days),
        DateUnit::Months | DateUnit::Years => unreachable!("calendar units are handled separately"),
    };

    duration.ok_or_else(arithmetic_overflow_error)
}

fn shift_date_by_months(date: NaiveDate, amount: i64, unit: DateUnit) -> Result<(NaiveDate, bool)> {
    let month_delta = match unit {
        DateUnit::Months => amount,
        DateUnit::Years => amount
            .checked_mul(12)
            .ok_or_else(arithmetic_overflow_error)?,
        _ => unreachable!("month shifting only supports months and years"),
    };
    let month_index = i64::from(date.year())
        .checked_mul(12)
        .and_then(|value| value.checked_add(i64::from(date.month0())))
        .and_then(|value| value.checked_add(month_delta))
        .ok_or_else(arithmetic_overflow_error)?;
    let year =
        i32::try_from(month_index.div_euclid(12)).map_err(|_| arithmetic_overflow_error())?;
    let month0 = u32::try_from(month_index.rem_euclid(12)).expect("month index is 0..=11");
    let month = month0 + 1;
    let target_day = date.day();
    let max_day = days_in_month(year, month)?;
    let day = target_day.min(max_day);
    let shifted =
        NaiveDate::from_ymd_opt(year, month, day).ok_or_else(arithmetic_overflow_error)?;
    Ok((shifted, day != target_day))
}

fn parse_temporal_with_hint(input: &str, input_format: Option<&str>) -> Result<Temporal> {
    if let Some(pattern) = input_format {
        if let Ok(datetime) = DateTime::parse_from_str(input, pattern) {
            return Ok(Temporal::OffsetDateTime {
                datetime,
                clamped: false,
            });
        }
        if let Ok(datetime) = NaiveDateTime::parse_from_str(input, pattern) {
            return Ok(Temporal::NaiveDateTime {
                datetime,
                clamped: false,
            });
        }
        if let Ok(date) = NaiveDate::parse_from_str(input, pattern) {
            return Ok(Temporal::Date {
                date,
                clamped: false,
            });
        }
        return Err(TimeKeepError::invalid_params(format!(
            "datetime does not match input format: {input}"
        ))
        .with_detail("parameter", json!("input_format"))
        .with_detail("input_format", json!(pattern))
        .with_detail("value", json!(input)));
    }

    parse_temporal(input)
}

fn parse_date_unit(input: &str) -> Result<DateUnit> {
    match input {
        "second" | "seconds" => Ok(DateUnit::Seconds),
        "minute" | "minutes" => Ok(DateUnit::Minutes),
        "hour" | "hours" => Ok(DateUnit::Hours),
        "day" | "days" => Ok(DateUnit::Days),
        "week" | "weeks" => Ok(DateUnit::Weeks),
        "month" | "months" => Ok(DateUnit::Months),
        "year" | "years" => Ok(DateUnit::Years),
        _ => Err(
            TimeKeepError::invalid_params(format!("invalid date unit: {input}"))
                .with_detail("parameter", json!("unit"))
                .with_detail("value", json!(input))
                .with_detail(
                    "allowed",
                    json!([
                        "seconds", "minutes", "hours", "days", "weeks", "months", "years"
                    ]),
                ),
        ),
    }
}

fn parse_temporal(input: &str) -> Result<Temporal> {
    if let Some(datetime) = parse_offset_datetime(input) {
        return Ok(Temporal::OffsetDateTime {
            datetime,
            clamped: false,
        });
    }
    for pattern in [
        DATETIME_T_FRACTIONAL_PATTERN,
        DATETIME_T_PATTERN,
        DATETIME_SPACE_FRACTIONAL_PATTERN,
        DATETIME_SPACE_PATTERN,
    ] {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(input, pattern) {
            return Ok(Temporal::NaiveDateTime {
                datetime,
                clamped: false,
            });
        }
    }
    if let Ok(date) = NaiveDate::parse_from_str(input, DATE_PATTERN) {
        return Ok(Temporal::Date {
            date,
            clamped: false,
        });
    }
    Err(invalid_date_error(input))
}

fn parse_offset_datetime(input: &str) -> Option<DateTime<FixedOffset>> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(input) {
        return Some(datetime);
    }

    let normalized_z;
    let candidate = if let Some(prefix) = input.strip_suffix('Z') {
        normalized_z = format!("{prefix}+00:00");
        normalized_z.as_str()
    } else {
        input
    };

    for pattern in [
        DATETIME_T_FRACTIONAL_OFFSET_PATTERN,
        DATETIME_T_OFFSET_PATTERN,
    ] {
        if let Ok(datetime) = DateTime::parse_from_str(candidate, pattern) {
            return Some(datetime);
        }
    }

    None
}

fn parse_date(input: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(input, DATE_PATTERN).map_err(|_| invalid_date_error(input))
}

fn invalid_date_error(input: &str) -> TimeKeepError {
    TimeKeepError::invalid_params(format!("invalid ISO date or datetime: {input}"))
        .with_detail("parameter", json!("date"))
        .with_detail("value", json!(input))
}

fn invalid_strftime_error(pattern: &str) -> TimeKeepError {
    TimeKeepError::invalid_params(format!("invalid strftime pattern: {pattern}"))
        .with_detail("parameter", json!("strftime"))
        .with_detail("value", json!(pattern))
}

fn rfc3339_year_error(year: i32) -> TimeKeepError {
    TimeKeepError::invalid_params("rfc3339 output supports years 0..=9999")
        .with_detail("output_format", json!("rfc3339"))
        .with_detail("year", json!(year))
}

fn rfc2822_year_error(year: i32) -> TimeKeepError {
    TimeKeepError::invalid_params("rfc2822 output supports years 0..=9999")
        .with_detail("output_format", json!("rfc2822"))
        .with_detail("year", json!(year))
}

fn arithmetic_overflow_error() -> TimeKeepError {
    TimeKeepError::invalid_params("date arithmetic overflowed supported range")
}

fn temporal_ordering(from: NaiveDateTime, to: NaiveDateTime) -> i64 {
    match to.cmp(&from) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn utc_fixed_offset() -> FixedOffset {
    FixedOffset::east_opt(0).expect("zero offset is valid")
}

fn date_unit_name(unit: DateUnit) -> &'static str {
    match unit {
        DateUnit::Seconds => "seconds",
        DateUnit::Minutes => "minutes",
        DateUnit::Hours => "hours",
        DateUnit::Days => "days",
        DateUnit::Weeks => "weeks",
        DateUnit::Months => "months",
        DateUnit::Years => "years",
    }
}

fn date_output_format_name(format: DateOutputFormat) -> &'static str {
    match format {
        DateOutputFormat::Iso8601 => "iso8601",
        DateOutputFormat::Rfc3339 => "rfc3339",
        DateOutputFormat::Rfc2822 => "rfc2822",
        DateOutputFormat::Epoch => "epoch",
        DateOutputFormat::UnixTimestamp => "unix_timestamp",
        DateOutputFormat::Strftime => "strftime",
    }
}

fn format_date(date: NaiveDate) -> String {
    date.format(DATE_PATTERN).to_string()
}

fn format_naive_datetime(datetime: NaiveDateTime) -> String {
    datetime.format(DATETIME_T_FRACTIONAL_PATTERN).to_string()
}

fn format_offset_datetime(datetime: DateTime<FixedOffset>) -> String {
    datetime.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true)
}

fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    }
}

fn days_in_month(year: i32, month: u32) -> Result<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Ok(31),
        4 | 6 | 9 | 11 => Ok(30),
        2 if is_leap_year(year) => Ok(29),
        2 => Ok(28),
        _ => Err(arithmetic_overflow_error()),
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_query_reports_expected_fields() {
        let result = calendar_query("2026-06-18").expect("calendar query");
        assert_eq!(result.weekday, "Thursday");
        assert_eq!(result.iso_week, 25);
        assert_eq!(result.day_of_year, 169);
        assert_eq!(result.days_in_month, 30);
        assert_eq!(result.quarter, 2);
    }

    #[test]
    fn calendar_query_handles_large_supported_december_years() {
        let result = calendar_query("+262142-12-01").expect("calendar query");
        assert_eq!(result.date, "+262142-12-01");
        assert_eq!(result.days_in_month, 31);

        let result = add("+262142-12-01", 0, "months").expect("date add");
        assert_eq!(result.result, "+262142-12-01");
        assert!(!result.month_end_clamped);
    }

    #[test]
    fn date_arithmetic_clamps_month_end() {
        let result = add("2026-01-31", 1, "month").expect("date add");
        assert_eq!(result.result, "2026-02-28");
        assert!(result.month_end_clamped);
    }

    #[test]
    fn date_arithmetic_clamps_leap_day_years() {
        let result = add("2024-02-29", 1, "year").expect("date add");
        assert_eq!(result.result, "2025-02-28");
        assert!(result.month_end_clamped);
    }

    #[test]
    fn date_arithmetic_rejects_oversized_duration_amounts() {
        let err = add("2026-06-18T00:00:00", i64::MAX, "seconds")
            .expect_err("oversized seconds should not panic");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");

        let err = add("2026-06-18", i64::MAX, "days").expect_err("oversized days should not panic");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");

        let err =
            add("2026-06-18", i64::MAX, "weeks").expect_err("oversized weeks should not panic");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn date_arithmetic_rejects_out_of_range_offset_month_shift() {
        let err = add("+262142-10-31T23:30:00-01:00", 2, "months")
            .expect_err("offset month shift should not panic");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn date_arithmetic_rejects_out_of_range_offset_duration_shift() {
        let err = add("+262142-12-31T23:00:00+01:00", 1, "hour")
            .expect_err("offset duration shift should not panic");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn invalid_date_unit_is_invalid_params() {
        let err = add("2026-01-31", 1, "fortnight").expect_err("invalid unit");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn date_diff_is_signed() {
        let result = diff("2026-06-18", "2026-06-01").expect("date diff");
        assert_eq!(result.signed_days, -17);
        assert_eq!(result.direction, -1);
        assert_eq!(result.absolute_seconds, 1_468_800);
    }

    #[test]
    fn date_diff_preserves_subsecond_direction_before_truncating_seconds() {
        let result = diff("2026-06-18T12:00:00.900", "2026-06-18T12:00:01.100").expect("date diff");
        assert_eq!(result.signed_seconds, 0);
        assert_eq!(result.direction, 1);
        assert_eq!(result.absolute_seconds, 0);

        let result = diff("2026-06-18T12:00:01.100", "2026-06-18T12:00:00.900").expect("date diff");
        assert_eq!(result.signed_seconds, 0);
        assert_eq!(result.direction, -1);
        assert_eq!(result.absolute_seconds, 0);
    }

    #[test]
    fn date_format_applies_utc_default_for_naive_rfc2822() {
        let result = format_datetime("2026-06-18T12:00:00", DateOutputFormat::Rfc2822, None, None)
            .expect("format datetime");
        assert_eq!(result.formatted, "Thu, 18 Jun 2026 12:00:00 +0000");
        assert!(!result.timezone_present);
    }

    #[test]
    fn date_format_applies_utc_default_for_naive_epoch() {
        let result = format_datetime("2026-06-18T12:00:00", DateOutputFormat::Epoch, None, None)
            .expect("format datetime");
        assert_eq!(result.formatted, "1781784000");
        assert!(!result.timezone_present);
    }

    #[test]
    fn date_format_outputs_rfc2822_for_absolute_input() {
        let result = format_datetime(
            "2026-06-18T12:00:00Z",
            DateOutputFormat::Rfc2822,
            None,
            None,
        )
        .expect("format datetime");
        assert_eq!(result.formatted, "Thu, 18 Jun 2026 12:00:00 +0000");
        assert!(result.timezone_present);
    }

    #[test]
    fn date_format_rejects_rfc2822_years_outside_supported_range() {
        let err = format_datetime("+10000-01-01", DateOutputFormat::Rfc2822, None, None)
            .expect_err("rfc2822 unsupported year");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn date_format_rejects_rfc3339_years_outside_supported_range() {
        let err = format_datetime("+10000-01-01", DateOutputFormat::Rfc3339, None, None)
            .expect_err("rfc3339 unsupported year");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }

    #[test]
    fn date_format_allows_expanded_years_for_iso8601() {
        let result = format_datetime("+10000-01-01", DateOutputFormat::Iso8601, None, None)
            .expect("iso8601 expanded year");
        assert_eq!(result.formatted, "+10000-01-01T00:00:00Z");
    }

    #[test]
    fn date_format_parses_expanded_offset_iso8601_by_default() {
        let result = format_datetime(
            "+10000-01-01T12:00:00+02:00",
            DateOutputFormat::Iso8601,
            None,
            None,
        )
        .expect("expanded offset datetime");
        assert_eq!(result.formatted, "+10000-01-01T12:00:00+02:00");
        assert!(result.timezone_present);
    }

    #[test]
    fn date_format_parses_expanded_zulu_iso8601_by_default() {
        let result = format_datetime(
            "+10000-01-01T12:00:00.500Z",
            DateOutputFormat::Iso8601,
            None,
            None,
        )
        .expect("expanded zulu datetime");
        assert_eq!(result.formatted, "+10000-01-01T12:00:00.500Z");
        assert!(result.timezone_present);
    }

    #[test]
    fn date_format_preserves_fractional_naive_datetime() {
        let result = format_datetime(
            "2026-06-18T12:00:00.500",
            DateOutputFormat::Iso8601,
            None,
            None,
        )
        .expect("format datetime");
        assert_eq!(result.formatted, "2026-06-18T12:00:00.500Z");
        assert!(!result.timezone_present);
    }

    #[test]
    fn strftime_formats_naive_dates_without_timezone() {
        let result = format_datetime(
            "2026-06-18",
            DateOutputFormat::Strftime,
            Some("%A %Y-%m-%d"),
            None,
        )
        .expect("strftime date");
        assert_eq!(result.formatted, "Thursday 2026-06-18");
        assert!(!result.timezone_present);
    }

    #[test]
    fn strftime_rejects_invalid_user_patterns() {
        let err = format_datetime("2026-06-18", DateOutputFormat::Strftime, Some("%"), None)
            .expect_err("invalid strftime pattern");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
    }
}
