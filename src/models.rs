use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::error::TimeKeepError;

#[derive(Debug, Serialize)]
pub(crate) struct ConfigPaths {
    pub(crate) config_path: String,
    pub(crate) data_dir: String,
    pub(crate) timer_db_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimeResponse {
    pub(crate) generated_at_utc: String,
    pub(crate) format: String,
    pub(crate) times: Vec<TimeSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimeSnapshot {
    pub(crate) timezone: String,
    pub(crate) local_datetime: String,
    pub(crate) utc_datetime: String,
    pub(crate) display_datetime: String,
    pub(crate) utc_offset: String,
    pub(crate) utc_offset_seconds: i32,
    pub(crate) is_dst: bool,
    pub(crate) abbreviation: Option<String>,
    pub(crate) weekday: String,
    pub(crate) iso_week: u32,
    pub(crate) iso_year: i32,
    pub(crate) day_of_year: u32,
    pub(crate) unix_epoch: i64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimezoneList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) region: Option<String>,
    pub(crate) count: usize,
    pub(crate) timezones: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimezoneInfo {
    pub(crate) timezone: String,
    pub(crate) current: TimeSnapshot,
    pub(crate) next_transition: Option<TimezoneTransition>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimezoneTransition {
    pub(crate) transition_utc: String,
    pub(crate) transition_local: String,
    pub(crate) offset_before: String,
    pub(crate) offset_before_seconds: i32,
    pub(crate) offset_after: String,
    pub(crate) offset_after_seconds: i32,
    pub(crate) abbreviation_before: Option<String>,
    pub(crate) abbreviation_after: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimezoneConversion {
    pub(crate) input_datetime: String,
    pub(crate) from_timezone: String,
    pub(crate) to_timezone: String,
    pub(crate) source: TimeSnapshot,
    pub(crate) target: TimeSnapshot,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CalendarQuery {
    pub(crate) date: String,
    pub(crate) weekday: String,
    pub(crate) weekday_number_from_monday: u32,
    pub(crate) iso_week: u32,
    pub(crate) iso_year: i32,
    pub(crate) day_of_year: u32,
    pub(crate) days_in_month: u32,
    pub(crate) leap_year: bool,
    pub(crate) quarter: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DateArithmetic {
    pub(crate) input: String,
    pub(crate) input_kind: String,
    pub(crate) operation: String,
    pub(crate) amount: i64,
    pub(crate) unit: String,
    pub(crate) result: String,
    pub(crate) result_kind: String,
    pub(crate) month_end_clamped: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DateDiff {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) from_kind: String,
    pub(crate) to_kind: String,
    pub(crate) signed_seconds: i64,
    pub(crate) signed_minutes: i64,
    pub(crate) signed_hours: i64,
    pub(crate) signed_days: i64,
    pub(crate) signed_weeks: i64,
    pub(crate) direction: i64,
    pub(crate) absolute_seconds: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DateFormatResult {
    pub(crate) input: String,
    pub(crate) input_kind: String,
    pub(crate) output_format: String,
    pub(crate) formatted: String,
    pub(crate) timezone_present: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HolidayCoverage {
    pub(crate) source: String,
    pub(crate) start_year: i32,
    pub(crate) end_year: i32,
    pub(crate) runtime_network: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HolidayEntry {
    pub(crate) country_code: String,
    pub(crate) country: String,
    pub(crate) date: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HolidayCheck {
    pub(crate) date: String,
    pub(crate) country_code: String,
    pub(crate) country: String,
    pub(crate) is_holiday: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) holiday: Option<HolidayEntry>,
    pub(crate) coverage: HolidayCoverage,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HolidayList {
    pub(crate) year: i32,
    pub(crate) country_code: String,
    pub(crate) country: String,
    pub(crate) count: usize,
    pub(crate) holidays: Vec<HolidayEntry>,
    pub(crate) coverage: HolidayCoverage,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BusinessDayCount {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) inclusive: bool,
    pub(crate) direction: i64,
    pub(crate) calendar_days: i64,
    pub(crate) business_days: i64,
    pub(crate) mode: String,
    pub(crate) country_code: Option<String>,
    pub(crate) skip_holidays: bool,
    pub(crate) holidays_skipped: Vec<HolidayEntry>,
    pub(crate) coverage: Option<HolidayCoverage>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BusinessDaySearch {
    pub(crate) input_date: String,
    pub(crate) business_date: String,
    pub(crate) direction: String,
    pub(crate) strict: bool,
    pub(crate) days_moved: i64,
    pub(crate) mode: String,
    pub(crate) country_code: Option<String>,
    pub(crate) coverage: Option<HolidayCoverage>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimerRecord {
    pub(crate) name: String,
    pub(crate) deadline_utc: String,
    pub(crate) original_deadline: String,
    pub(crate) timezone: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) created_at_utc: String,
    pub(crate) updated_at_utc: String,
    pub(crate) status: String,
    pub(crate) overdue: bool,
    pub(crate) remaining_seconds: i64,
    pub(crate) remaining: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimerList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tag: Option<String>,
    pub(crate) count: usize,
    pub(crate) timers: Vec<TimerRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimerDelete {
    pub(crate) name: String,
    pub(crate) deleted: bool,
    pub(crate) deleted_tags: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TimerCheck {
    pub(crate) generated_at_utc: String,
    pub(crate) count: usize,
    pub(crate) timers: Vec<TimerRecord>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ErrorEnvelope<'a> {
    pub(crate) error: ErrorBody<'a>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ErrorBody<'a> {
    pub(crate) error_code: &'static str,
    pub(crate) message: &'a str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) details: &'a BTreeMap<String, Value>,
}

impl<'a> From<&'a TimeKeepError> for ErrorEnvelope<'a> {
    fn from(err: &'a TimeKeepError) -> Self {
        Self {
            error: ErrorBody {
                error_code: err.code().as_str(),
                message: err.message(),
                details: err.details(),
            },
        }
    }
}
