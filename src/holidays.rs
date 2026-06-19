use std::{
    collections::{BTreeMap, btree_map::Entry},
    str::FromStr,
};

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use serde_json::json;

use crate::{
    error::{Result, TimeKeepError},
    models::{
        BusinessDayCount, BusinessDaySearch, HolidayCheck, HolidayCoverage, HolidayEntry,
        HolidayList,
    },
};

const DATE_PATTERN: &str = "%Y-%m-%d";
const COVERAGE_START_YEAR: i32 = 2000;
const COVERAGE_END_YEAR: i32 = 2030;
const COVERAGE_SOURCE: &str = "holidays crate offline generated data";
const MAX_BUSINESS_DAY_SEARCH_DAYS: i64 = 366 * 2;

const SUPPORTED_COUNTRIES: &[&str] = &[
    "AE", "AM", "AO", "AR", "AT", "AU", "AW", "AZ", "BA", "BD", "BE", "BG", "BI", "BO", "BR", "BW",
    "BY", "CA", "CH", "CL", "CN", "CO", "CU", "CW", "CY", "CZ", "DE", "DJ", "DK", "DO", "EE", "EG",
    "ES", "ET", "FI", "FR", "GB", "GE", "GR", "HK", "HN", "HR", "HU", "ID", "IE", "IL", "IM", "IN",
    "IS", "IT", "JM", "JP", "KE", "KR", "KZ", "LI", "LS", "LT", "LU", "LV", "MA", "MD", "MG", "MK",
    "MT", "MW", "MX", "MY", "MZ", "NA", "NG", "NI", "NL", "NO", "NZ", "PE", "PK", "PL", "PT", "PY",
    "RO", "RS", "RU", "SA", "SE", "SG", "SI", "SK", "SZ", "TN", "TR", "TW", "UA", "US", "UY", "UZ",
    "VE", "VN", "ZA", "ZM", "ZW",
];

type HolidayMap = BTreeMap<NaiveDate, HolidayEntry>;

pub(crate) fn holiday_check(input: &str, country: &str) -> Result<HolidayCheck> {
    let date = parse_date(input)?;
    validate_year(date.year())?;
    let country = parse_country(country)?;
    let country_code = country.to_string();
    let holidays = holidays_for_year(country, date.year())?;
    let country_name = country_name(&country_code, &holidays);
    let holiday = holidays.get(&date).cloned();

    Ok(HolidayCheck {
        date: format_date(date),
        country_code,
        country: country_name,
        is_holiday: holiday.is_some(),
        holiday,
        coverage: coverage(),
    })
}

pub(crate) fn holiday_list(year: i32, country: &str) -> Result<HolidayList> {
    validate_year(year)?;
    let country = parse_country(country)?;
    let country_code = country.to_string();
    let holidays = holidays_for_year(country, year)?;
    let country_name = country_name(&country_code, &holidays);
    let holidays = holidays.into_values().collect::<Vec<_>>();

    Ok(HolidayList {
        year,
        country_code,
        country: country_name,
        count: holidays.len(),
        holidays,
        coverage: coverage(),
    })
}

pub(crate) fn business_days_between(
    from: &str,
    to: &str,
    country: Option<&str>,
    skip_holidays: bool,
) -> Result<BusinessDayCount> {
    let from_date = parse_date(from)?;
    let to_date = parse_date(to)?;
    let direction = if from_date <= to_date { 1 } else { -1 };
    let start = from_date.min(to_date);
    let end = from_date.max(to_date);
    let calendar_days = end.signed_duration_since(start).num_days() + 1;

    let country = parse_optional_country(country)?;
    let (mode, holiday_map, coverage) = if skip_holidays {
        let country = country.ok_or_else(skip_holidays_without_country_error)?;
        validate_year_range(start.year(), end.year())?;
        (
            "country_holidays",
            Some(holidays_for_year_range(country, start.year(), end.year())?),
            Some(coverage()),
        )
    } else {
        ("weekend_only", None, None)
    };

    let mut business_days = count_weekdays_inclusive(start, end);
    let holidays_skipped = holiday_map
        .as_ref()
        .map(|holidays| {
            holidays
                .values()
                .filter(|holiday| {
                    let date = parse_trusted_date(&holiday.date);
                    start <= date && date <= end && !is_weekend(date)
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    business_days -= i64::try_from(holidays_skipped.len()).map_err(|_| {
        TimeKeepError::invalid_params("business-day holiday count overflowed supported range")
    })?;

    Ok(BusinessDayCount {
        from: format_date(from_date),
        to: format_date(to_date),
        inclusive: true,
        direction,
        calendar_days: calendar_days * direction,
        business_days: business_days * direction,
        mode: mode.to_string(),
        country_code: country.map(|country| country.to_string()),
        skip_holidays,
        holidays_skipped,
        coverage,
    })
}

pub(crate) fn next_business_day(input: &str, country: Option<&str>) -> Result<BusinessDaySearch> {
    search_business_day(input, country, SearchDirection::Next)
}

pub(crate) fn previous_business_day(
    input: &str,
    country: Option<&str>,
) -> Result<BusinessDaySearch> {
    search_business_day(input, country, SearchDirection::Previous)
}

fn search_business_day(
    input: &str,
    country: Option<&str>,
    direction: SearchDirection,
) -> Result<BusinessDaySearch> {
    let date = parse_date(input)?;
    let country = parse_optional_country(country)?;
    let mode = if country.is_some() {
        validate_year(date.year())?;
        "country_holidays"
    } else {
        "weekend_only"
    };
    let mut loaded_years = BTreeMap::new();

    for days_moved in 1..=MAX_BUSINESS_DAY_SEARCH_DAYS {
        let candidate = date
            .checked_add_signed(Duration::days(days_moved * direction.sign()))
            .ok_or_else(search_overflow_error)?;
        if let Some(country) = country {
            validate_year(candidate.year())?;
            if let Entry::Vacant(entry) = loaded_years.entry(candidate.year()) {
                entry.insert(holidays_for_year(country, candidate.year())?);
            }
        }
        let holiday_map = loaded_years.get(&candidate.year());
        if is_business_day(candidate, holiday_map) {
            return Ok(BusinessDaySearch {
                input_date: format_date(date),
                business_date: format_date(candidate),
                direction: direction.name().to_string(),
                strict: true,
                days_moved,
                mode: mode.to_string(),
                country_code: country.map(|country| country.to_string()),
                coverage: country.map(|_| coverage()),
            });
        }
    }

    Err(search_overflow_error())
}

fn holidays_for_year(country: ::holidays::Country, year: i32) -> Result<HolidayMap> {
    validate_year(year)?;
    let mut data = ::holidays::Builder::new()
        .countries(&[country])
        .years(year..(year + 1))
        .build()
        .map_err(|err| holiday_crate_error(err, Some(country), Some(year)))?;
    let Some(mut years) = data.remove(&country) else {
        return Err(unsupported_country_error(country.as_ref()));
    };
    let Some(holidays) = years.remove(&year) else {
        return Err(unsupported_year_error(year));
    };
    Ok(holidays
        .into_iter()
        .map(|(date, holiday)| (date, holiday_entry(holiday)))
        .collect())
}

fn holidays_for_year_range(
    country: ::holidays::Country,
    start_year: i32,
    end_year: i32,
) -> Result<HolidayMap> {
    validate_year_range(start_year, end_year)?;
    let mut data = ::holidays::Builder::new()
        .countries(&[country])
        .years(start_year..(end_year + 1))
        .build()
        .map_err(|err| holiday_crate_error(err, Some(country), None))?;
    let Some(years) = data.remove(&country) else {
        return Err(unsupported_country_error(country.as_ref()));
    };
    let mut holidays = HolidayMap::new();
    for year in start_year..=end_year {
        let Some(year_holidays) = years.get(&year) else {
            return Err(unsupported_year_error(year));
        };
        holidays.extend(
            year_holidays
                .values()
                .cloned()
                .map(|holiday| (holiday.date, holiday_entry(holiday))),
        );
    }
    Ok(holidays)
}

fn holiday_entry(holiday: ::holidays::Holiday) -> HolidayEntry {
    HolidayEntry {
        country_code: holiday.code.to_string(),
        country: holiday.country,
        date: format_date(holiday.date),
        name: holiday.name,
    }
}

fn parse_optional_country(country: Option<&str>) -> Result<Option<::holidays::Country>> {
    country.map(parse_country).transpose()
}

fn parse_country(input: &str) -> Result<::holidays::Country> {
    let normalized = input.trim().to_ascii_uppercase();
    if normalized.len() != 2 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(unsupported_country_error(input));
    }
    ::holidays::Country::from_str(&normalized).map_err(|_| unsupported_country_error(input))
}

fn parse_date(input: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(input, DATE_PATTERN).map_err(|_| {
        TimeKeepError::invalid_params(format!("invalid ISO date: {input}"))
            .with_detail("parameter", json!("date"))
            .with_detail("value", json!(input))
    })
}

fn parse_trusted_date(input: &str) -> NaiveDate {
    NaiveDate::parse_from_str(input, DATE_PATTERN).expect("holiday adapter emits ISO dates")
}

fn validate_year(year: i32) -> Result<()> {
    if (COVERAGE_START_YEAR..=COVERAGE_END_YEAR).contains(&year) {
        Ok(())
    } else {
        Err(unsupported_year_error(year))
    }
}

fn validate_year_range(start_year: i32, end_year: i32) -> Result<()> {
    validate_year(start_year)?;
    validate_year(end_year)
}

fn count_weekdays_inclusive(start: NaiveDate, end: NaiveDate) -> i64 {
    let calendar_days = end.signed_duration_since(start).num_days() + 1;
    let full_weeks = calendar_days / 7;
    let mut weekdays = full_weeks * 5;
    let remaining_days = calendar_days % 7;

    for day_offset in 0..remaining_days {
        let date = start + Duration::days(day_offset);
        if !is_weekend(date) {
            weekdays += 1;
        }
    }

    weekdays
}

fn is_business_day(date: NaiveDate, holidays: Option<&HolidayMap>) -> bool {
    !is_weekend(date) && holidays.is_none_or(|holidays| !holidays.contains_key(&date))
}

fn is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

fn coverage() -> HolidayCoverage {
    HolidayCoverage {
        source: COVERAGE_SOURCE.to_string(),
        start_year: COVERAGE_START_YEAR,
        end_year: COVERAGE_END_YEAR,
        runtime_network: false,
    }
}

fn country_name(country_code: &str, holidays: &HolidayMap) -> String {
    holidays
        .values()
        .next()
        .map(|holiday| holiday.country.clone())
        .unwrap_or_else(|| country_code.to_string())
}

fn format_date(date: NaiveDate) -> String {
    date.format(DATE_PATTERN).to_string()
}

fn unsupported_country_error(input: &str) -> TimeKeepError {
    TimeKeepError::invalid_params(format!("unsupported holiday country: {input}"))
        .with_detail("parameter", json!("country"))
        .with_detail("value", json!(input))
        .with_detail("supported_countries", json!(SUPPORTED_COUNTRIES))
        .with_detail("coverage_start_year", json!(COVERAGE_START_YEAR))
        .with_detail("coverage_end_year", json!(COVERAGE_END_YEAR))
        .with_detail("runtime_network", json!(false))
}

fn unsupported_year_error(year: i32) -> TimeKeepError {
    TimeKeepError::invalid_params(format!(
        "holiday data supports years {COVERAGE_START_YEAR}..={COVERAGE_END_YEAR}: {year}"
    ))
    .with_detail("parameter", json!("year"))
    .with_detail("value", json!(year))
    .with_detail("coverage_start_year", json!(COVERAGE_START_YEAR))
    .with_detail("coverage_end_year", json!(COVERAGE_END_YEAR))
    .with_detail("runtime_network", json!(false))
}

fn skip_holidays_without_country_error() -> TimeKeepError {
    TimeKeepError::invalid_params("--skip-holidays requires --country")
        .with_detail("parameter", json!("country"))
}

fn search_overflow_error() -> TimeKeepError {
    TimeKeepError::invalid_params("could not find a business day inside supported range")
        .with_detail("coverage_start_year", json!(COVERAGE_START_YEAR))
        .with_detail("coverage_end_year", json!(COVERAGE_END_YEAR))
}

fn holiday_crate_error(
    err: ::holidays::Error,
    country: Option<::holidays::Country>,
    year: Option<i32>,
) -> TimeKeepError {
    match err {
        ::holidays::Error::CountryNotAvailable => country
            .map(|country| unsupported_country_error(country.as_ref()))
            .unwrap_or_else(|| unsupported_country_error("")),
        ::holidays::Error::YearNotAvailable => {
            year.map(unsupported_year_error).unwrap_or_else(|| {
                TimeKeepError::invalid_params("holiday year is outside supported coverage")
                    .with_detail("coverage_start_year", json!(COVERAGE_START_YEAR))
                    .with_detail("coverage_end_year", json!(COVERAGE_END_YEAR))
            })
        }
        other => TimeKeepError::new(
            crate::error::ErrorCode::Internal,
            format!("holiday adapter failed: {other}"),
        ),
    }
}

#[derive(Debug, Clone, Copy)]
enum SearchDirection {
    Next,
    Previous,
}

impl SearchDirection {
    fn sign(self) -> i64 {
        match self {
            Self::Next => 1,
            Self::Previous => -1,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Next => "next",
            Self::Previous => "previous",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holiday_check_finds_us_christmas() {
        let result = holiday_check("2026-12-25", "us").expect("holiday check");
        assert!(result.is_holiday);
        assert_eq!(result.country_code, "US");
        assert_eq!(result.holiday.expect("holiday").name, "Christmas Day");
        assert!(!result.coverage.runtime_network);
    }

    #[test]
    fn holiday_list_loads_requested_country_and_year() {
        let result = holiday_list(2026, "GB").expect("holiday list");
        assert_eq!(result.country_code, "GB");
        assert_eq!(result.year, 2026);
        assert!(
            result
                .holidays
                .iter()
                .any(|holiday| holiday.date == "2026-12-25")
        );
    }

    #[test]
    fn holiday_year_outside_coverage_is_invalid_params() {
        let err = holiday_list(2031, "US").expect_err("unsupported year");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
        assert_eq!(err.details().get("coverage_end_year"), Some(&json!(2030)));
    }

    #[test]
    fn holiday_coverage_boundaries_are_documented_by_tests() {
        let start = holiday_list(2000, "US").expect("coverage start year");
        assert_eq!(start.coverage.start_year, 2000);
        assert_eq!(start.coverage.end_year, 2030);

        let end = holiday_list(2030, "US").expect("coverage end year");
        assert_eq!(end.coverage.start_year, 2000);
        assert_eq!(end.coverage.end_year, 2030);

        let before_start = holiday_list(1999, "US").expect_err("year before coverage");
        assert_eq!(before_start.code().as_str(), "INVALID_PARAMS");
        assert_eq!(
            before_start.details().get("coverage_start_year"),
            Some(&json!(2000))
        );
    }

    #[test]
    fn unsupported_country_is_invalid_params() {
        let err = holiday_check("2026-12-25", "XX").expect_err("unsupported country");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
        assert_eq!(err.details().get("parameter"), Some(&json!("country")));
    }

    #[test]
    fn business_days_between_is_weekend_only_without_skip_flag() {
        let result = business_days_between("2026-12-24", "2026-12-28", Some("US"), false)
            .expect("business days");
        assert_eq!(result.business_days, 3);
        assert_eq!(result.mode, "weekend_only");
        assert!(result.holidays_skipped.is_empty());
        assert!(result.coverage.is_none());
    }

    #[test]
    fn business_days_between_skips_country_holidays_when_enabled() {
        let result = business_days_between("2026-12-24", "2026-12-28", Some("US"), true)
            .expect("business days");
        assert_eq!(result.business_days, 2);
        assert_eq!(result.mode, "country_holidays");
        assert_eq!(result.holidays_skipped.len(), 1);
        assert_eq!(result.holidays_skipped[0].date, "2026-12-25");
    }

    #[test]
    fn business_days_between_supports_reverse_ranges() {
        let result =
            business_days_between("2026-12-28", "2026-12-24", Some("US"), true).expect("range");
        assert_eq!(result.direction, -1);
        assert_eq!(result.business_days, -2);
        assert_eq!(result.calendar_days, -5);
    }

    #[test]
    fn skip_holidays_requires_country() {
        let err = business_days_between("2026-12-24", "2026-12-28", None, true)
            .expect_err("country required");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
        assert_eq!(err.details().get("parameter"), Some(&json!("country")));
    }

    #[test]
    fn next_business_day_uses_strict_after_semantics() {
        let result = next_business_day("2026-12-25", Some("US")).expect("next business day");
        assert_eq!(result.business_date, "2026-12-28");
        assert_eq!(result.days_moved, 3);
    }

    #[test]
    fn previous_business_day_uses_strict_before_semantics() {
        let result =
            previous_business_day("2026-12-28", Some("US")).expect("previous business day");
        assert_eq!(result.business_date, "2026-12-24");
        assert_eq!(result.days_moved, 4);
    }

    #[test]
    fn weekend_only_next_business_day_does_not_require_coverage() {
        let result = next_business_day("2099-12-25", None).expect("weekend-only next");
        assert_eq!(result.mode, "weekend_only");
        assert!(result.coverage.is_none());
    }
}
