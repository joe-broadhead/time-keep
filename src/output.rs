use std::io::{self, Write};

use comfy_table::{Table, presets::UTF8_FULL};
use serde::Serialize;

use crate::{
    cli::OutputFormat,
    error::{Result, TimeKeepError},
    models::{
        BusinessDayCount, BusinessDaySearch, CalendarQuery, ConfigPaths, DateArithmetic, DateDiff,
        DateFormatResult, ErrorEnvelope, HolidayCheck, HolidayList, TimeResponse, TimerCheck,
        TimerDelete, TimerList, TimerRecord, TimezoneConversion, TimezoneInfo, TimezoneList,
    },
};

pub(crate) enum TableData<'a> {
    ConfigPaths(&'a ConfigPaths),
    TimeResponse(&'a TimeResponse),
    TimezoneList(&'a TimezoneList),
    TimezoneInfo(&'a TimezoneInfo),
    TimezoneConversion(&'a TimezoneConversion),
    CalendarQuery(&'a CalendarQuery),
    DateArithmetic(&'a DateArithmetic),
    DateDiff(&'a DateDiff),
    DateFormatResult(&'a DateFormatResult),
    HolidayCheck(&'a HolidayCheck),
    HolidayList(&'a HolidayList),
    BusinessDayCount(&'a BusinessDayCount),
    BusinessDaySearch(&'a BusinessDaySearch),
    TimerRecord(&'a TimerRecord),
    TimerList(&'a TimerList),
    TimerDelete(&'a TimerDelete),
    TimerCheck(&'a TimerCheck),
}

pub(crate) fn render<T: Serialize>(
    format: OutputFormat,
    value: &T,
    table_data: TableData<'_>,
) -> Result<()> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    render_to_writer(format, value, table_data, &mut writer)
}

pub(crate) fn render_error(format: OutputFormat, err: &TimeKeepError) -> Result<()> {
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    render_error_to_writer(format, err, &mut writer)
}

fn render_to_writer<T: Serialize, W: Write>(
    format: OutputFormat,
    value: &T,
    table_data: TableData<'_>,
    writer: &mut W,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, value)?;
            writeln!(writer)?;
        }
        OutputFormat::Table => {
            writeln!(writer, "{}", to_table(table_data))?;
        }
        OutputFormat::Csv => {
            write_csv(table_data, writer)?;
        }
    }
    Ok(())
}

fn render_error_to_writer<W: Write>(
    format: OutputFormat,
    err: &TimeKeepError,
    writer: &mut W,
) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let envelope = ErrorEnvelope::from(err);
            serde_json::to_writer_pretty(&mut *writer, &envelope)?;
            writeln!(writer)?;
        }
        OutputFormat::Table => {
            let mut table = Table::new();
            table.load_preset(UTF8_FULL);
            table.set_header(["Error Code", "Message"]);
            table.add_row([err.code().as_str(), err.message()]);
            writeln!(writer, "{table}")?;
        }
        OutputFormat::Csv => {
            let mut csv = csv::Writer::from_writer(writer);
            csv.write_record(["error_code", "message"])?;
            csv.write_record([err.code().as_str(), err.message()])?;
            csv.flush()?;
        }
    }
    Ok(())
}

fn to_table(data: TableData<'_>) -> String {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    match data {
        TableData::ConfigPaths(paths) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["config_path", paths.config_path.as_str()]);
            table.add_row(["data_dir", paths.data_dir.as_str()]);
            table.add_row(["timer_db_path", paths.timer_db_path.as_str()]);
        }
        TableData::TimeResponse(response) => {
            table.set_header([
                "Timezone",
                "Local Datetime",
                "UTC Offset",
                "DST",
                "Abbrev",
                "Epoch",
            ]);
            for time in &response.times {
                table.add_row(vec![
                    time.timezone.clone(),
                    time.display_datetime.clone(),
                    time.utc_offset.clone(),
                    time.is_dst.to_string(),
                    time.abbreviation.clone().unwrap_or_default(),
                    time.unix_epoch.to_string(),
                ]);
            }
        }
        TableData::TimezoneList(list) => {
            table.set_header(["Timezone"]);
            for timezone in &list.timezones {
                table.add_row([timezone.as_str()]);
            }
        }
        TableData::TimezoneInfo(info) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["timezone", info.timezone.as_str()]);
            table.add_row(["local_datetime", info.current.local_datetime.as_str()]);
            table.add_row(["utc_datetime", info.current.utc_datetime.as_str()]);
            table.add_row(["utc_offset", info.current.utc_offset.as_str()]);
            table.add_row(["is_dst", bool_text(info.current.is_dst)]);
            table.add_row([
                "abbreviation",
                info.current.abbreviation.as_deref().unwrap_or_default(),
            ]);
            if let Some(transition) = &info.next_transition {
                table.add_row(["next_transition_utc", transition.transition_utc.as_str()]);
                table.add_row([
                    "next_transition_local",
                    transition.transition_local.as_str(),
                ]);
                table.add_row(["offset_before", transition.offset_before.as_str()]);
                table.add_row(["offset_after", transition.offset_after.as_str()]);
            } else {
                table.add_row(["next_transition_utc", ""]);
            }
        }
        TableData::TimezoneConversion(conversion) => {
            table.set_header([
                "Role",
                "Timezone",
                "Local Datetime",
                "UTC Offset",
                "DST",
                "Abbrev",
            ]);
            table.add_row(vec![
                "source".to_string(),
                conversion.source.timezone.clone(),
                conversion.source.local_datetime.clone(),
                conversion.source.utc_offset.clone(),
                conversion.source.is_dst.to_string(),
                conversion.source.abbreviation.clone().unwrap_or_default(),
            ]);
            table.add_row(vec![
                "target".to_string(),
                conversion.target.timezone.clone(),
                conversion.target.local_datetime.clone(),
                conversion.target.utc_offset.clone(),
                conversion.target.is_dst.to_string(),
                conversion.target.abbreviation.clone().unwrap_or_default(),
            ]);
        }
        TableData::CalendarQuery(calendar) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["date", calendar.date.as_str()]);
            table.add_row(["weekday", calendar.weekday.as_str()]);
            table.add_row([
                "weekday_number_from_monday",
                &calendar.weekday_number_from_monday.to_string(),
            ]);
            table.add_row(["iso_week", &calendar.iso_week.to_string()]);
            table.add_row(["iso_year", &calendar.iso_year.to_string()]);
            table.add_row(["day_of_year", &calendar.day_of_year.to_string()]);
            table.add_row(["days_in_month", &calendar.days_in_month.to_string()]);
            table.add_row(["leap_year", bool_text(calendar.leap_year)]);
            table.add_row(["quarter", &calendar.quarter.to_string()]);
        }
        TableData::DateArithmetic(arithmetic) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["input", arithmetic.input.as_str()]);
            table.add_row(["input_kind", arithmetic.input_kind.as_str()]);
            table.add_row(["operation", arithmetic.operation.as_str()]);
            table.add_row(["amount", &arithmetic.amount.to_string()]);
            table.add_row(["unit", arithmetic.unit.as_str()]);
            table.add_row(["result", arithmetic.result.as_str()]);
            table.add_row(["result_kind", arithmetic.result_kind.as_str()]);
            table.add_row(["month_end_clamped", bool_text(arithmetic.month_end_clamped)]);
        }
        TableData::DateDiff(diff) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["from", diff.from.as_str()]);
            table.add_row(["to", diff.to.as_str()]);
            table.add_row(["from_kind", diff.from_kind.as_str()]);
            table.add_row(["to_kind", diff.to_kind.as_str()]);
            table.add_row(["signed_seconds", &diff.signed_seconds.to_string()]);
            table.add_row(["signed_minutes", &diff.signed_minutes.to_string()]);
            table.add_row(["signed_hours", &diff.signed_hours.to_string()]);
            table.add_row(["signed_days", &diff.signed_days.to_string()]);
            table.add_row(["signed_weeks", &diff.signed_weeks.to_string()]);
            table.add_row(["direction", &diff.direction.to_string()]);
            table.add_row(["absolute_seconds", &diff.absolute_seconds.to_string()]);
        }
        TableData::DateFormatResult(format) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["input", format.input.as_str()]);
            table.add_row(["input_kind", format.input_kind.as_str()]);
            table.add_row(["output_format", format.output_format.as_str()]);
            table.add_row(["formatted", format.formatted.as_str()]);
            table.add_row(["timezone_present", bool_text(format.timezone_present)]);
        }
        TableData::HolidayCheck(check) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["date", check.date.as_str()]);
            table.add_row(["country_code", check.country_code.as_str()]);
            table.add_row(["country", check.country.as_str()]);
            table.add_row(["is_holiday", bool_text(check.is_holiday)]);
            table.add_row([
                "holiday",
                check
                    .holiday
                    .as_ref()
                    .map(|holiday| holiday.name.as_str())
                    .unwrap_or_default(),
            ]);
            add_coverage_rows(&mut table, &check.coverage);
        }
        TableData::HolidayList(list) => {
            table.set_header(["Date", "Country", "Name"]);
            for holiday in &list.holidays {
                table.add_row([
                    holiday.date.as_str(),
                    holiday.country_code.as_str(),
                    holiday.name.as_str(),
                ]);
            }
        }
        TableData::BusinessDayCount(count) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["from", count.from.as_str()]);
            table.add_row(["to", count.to.as_str()]);
            table.add_row(["inclusive", bool_text(count.inclusive)]);
            table.add_row(["direction", &count.direction.to_string()]);
            table.add_row(["calendar_days", &count.calendar_days.to_string()]);
            table.add_row(["business_days", &count.business_days.to_string()]);
            table.add_row(["mode", count.mode.as_str()]);
            table.add_row(["skip_holidays", bool_text(count.skip_holidays)]);
            table.add_row([
                "country_code",
                count.country_code.as_deref().unwrap_or_default(),
            ]);
            table.add_row([
                "holidays_skipped",
                &count.holidays_skipped.len().to_string(),
            ]);
            if let Some(coverage) = &count.coverage {
                add_coverage_rows(&mut table, coverage);
            }
        }
        TableData::BusinessDaySearch(search) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["input_date", search.input_date.as_str()]);
            table.add_row(["business_date", search.business_date.as_str()]);
            table.add_row(["direction", search.direction.as_str()]);
            table.add_row(["strict", bool_text(search.strict)]);
            table.add_row(["days_moved", &search.days_moved.to_string()]);
            table.add_row(["mode", search.mode.as_str()]);
            table.add_row([
                "country_code",
                search.country_code.as_deref().unwrap_or_default(),
            ]);
            if let Some(coverage) = &search.coverage {
                add_coverage_rows(&mut table, coverage);
            }
        }
        TableData::TimerRecord(timer) => add_timer_record_rows(&mut table, timer),
        TableData::TimerList(list) => {
            table.set_header(["Name", "Deadline UTC", "Status", "Remaining", "Tags"]);
            for timer in &list.timers {
                table.add_row([
                    timer.name.as_str(),
                    timer.deadline_utc.as_str(),
                    timer.status.as_str(),
                    timer.remaining.as_str(),
                    &timer.tags.join(","),
                ]);
            }
        }
        TableData::TimerDelete(deleted) => {
            table.set_header(["Field", "Value"]);
            table.add_row(["name", deleted.name.as_str()]);
            table.add_row(["deleted", bool_text(deleted.deleted)]);
            table.add_row(["deleted_tags", &deleted.deleted_tags.to_string()]);
        }
        TableData::TimerCheck(check) => {
            table.set_header(["Name", "Deadline UTC", "Status", "Remaining", "Tags"]);
            for timer in &check.timers {
                table.add_row([
                    timer.name.as_str(),
                    timer.deadline_utc.as_str(),
                    timer.status.as_str(),
                    timer.remaining.as_str(),
                    &timer.tags.join(","),
                ]);
            }
        }
    }
    table.to_string()
}

fn add_timer_record_rows(table: &mut Table, timer: &TimerRecord) {
    table.set_header(["Field", "Value"]);
    table.add_row(["name", timer.name.as_str()]);
    table.add_row(["deadline_utc", timer.deadline_utc.as_str()]);
    table.add_row(["original_deadline", timer.original_deadline.as_str()]);
    table.add_row(["timezone", timer.timezone.as_str()]);
    table.add_row([
        "description",
        timer.description.as_deref().unwrap_or_default(),
    ]);
    table.add_row(["tags", &timer.tags.join(",")]);
    table.add_row(["created_at_utc", timer.created_at_utc.as_str()]);
    table.add_row(["updated_at_utc", timer.updated_at_utc.as_str()]);
    table.add_row(["status", timer.status.as_str()]);
    table.add_row(["overdue", bool_text(timer.overdue)]);
    table.add_row(["remaining_seconds", &timer.remaining_seconds.to_string()]);
    table.add_row(["remaining", timer.remaining.as_str()]);
}

fn add_coverage_rows(table: &mut Table, coverage: &crate::models::HolidayCoverage) {
    table.add_row(["coverage_source", coverage.source.as_str()]);
    table.add_row(["coverage_start_year", &coverage.start_year.to_string()]);
    table.add_row(["coverage_end_year", &coverage.end_year.to_string()]);
    table.add_row(["runtime_network", bool_text(coverage.runtime_network)]);
}

fn bool_text(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn write_csv<W: Write>(data: TableData<'_>, writer: W) -> Result<()> {
    let mut csv = csv::Writer::from_writer(writer);
    match data {
        TableData::ConfigPaths(paths) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["config_path", paths.config_path.as_str()])?;
            csv.write_record(["data_dir", paths.data_dir.as_str()])?;
            csv.write_record(["timer_db_path", paths.timer_db_path.as_str()])?;
        }
        TableData::TimeResponse(response) => {
            csv.write_record([
                "timezone",
                "local_datetime",
                "utc_datetime",
                "display_datetime",
                "utc_offset",
                "utc_offset_seconds",
                "is_dst",
                "abbreviation",
                "weekday",
                "iso_week",
                "iso_year",
                "day_of_year",
                "unix_epoch",
            ])?;
            for time in &response.times {
                csv.write_record([
                    time.timezone.as_str(),
                    time.local_datetime.as_str(),
                    time.utc_datetime.as_str(),
                    time.display_datetime.as_str(),
                    time.utc_offset.as_str(),
                    &time.utc_offset_seconds.to_string(),
                    bool_text(time.is_dst),
                    time.abbreviation.as_deref().unwrap_or_default(),
                    time.weekday.as_str(),
                    &time.iso_week.to_string(),
                    &time.iso_year.to_string(),
                    &time.day_of_year.to_string(),
                    &time.unix_epoch.to_string(),
                ])?;
            }
        }
        TableData::TimezoneList(list) => {
            csv.write_record(["timezone"])?;
            for timezone in &list.timezones {
                csv.write_record([timezone.as_str()])?;
            }
        }
        TableData::TimezoneInfo(info) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["timezone", info.timezone.as_str()])?;
            csv.write_record(["local_datetime", info.current.local_datetime.as_str()])?;
            csv.write_record(["utc_datetime", info.current.utc_datetime.as_str()])?;
            csv.write_record(["utc_offset", info.current.utc_offset.as_str()])?;
            csv.write_record(["is_dst", bool_text(info.current.is_dst)])?;
            csv.write_record([
                "abbreviation",
                info.current.abbreviation.as_deref().unwrap_or_default(),
            ])?;
            if let Some(transition) = &info.next_transition {
                csv.write_record(["next_transition_utc", transition.transition_utc.as_str()])?;
                csv.write_record([
                    "next_transition_local",
                    transition.transition_local.as_str(),
                ])?;
                csv.write_record(["offset_before", transition.offset_before.as_str()])?;
                csv.write_record(["offset_after", transition.offset_after.as_str()])?;
            }
        }
        TableData::TimezoneConversion(conversion) => {
            csv.write_record([
                "role",
                "timezone",
                "local_datetime",
                "utc_offset",
                "is_dst",
                "abbreviation",
            ])?;
            for (role, time) in [
                ("source", &conversion.source),
                ("target", &conversion.target),
            ] {
                csv.write_record([
                    role,
                    time.timezone.as_str(),
                    time.local_datetime.as_str(),
                    time.utc_offset.as_str(),
                    bool_text(time.is_dst),
                    time.abbreviation.as_deref().unwrap_or_default(),
                ])?;
            }
        }
        TableData::CalendarQuery(calendar) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["date", calendar.date.as_str()])?;
            csv.write_record(["weekday", calendar.weekday.as_str()])?;
            csv.write_record([
                "weekday_number_from_monday",
                &calendar.weekday_number_from_monday.to_string(),
            ])?;
            csv.write_record(["iso_week", &calendar.iso_week.to_string()])?;
            csv.write_record(["iso_year", &calendar.iso_year.to_string()])?;
            csv.write_record(["day_of_year", &calendar.day_of_year.to_string()])?;
            csv.write_record(["days_in_month", &calendar.days_in_month.to_string()])?;
            csv.write_record(["leap_year", bool_text(calendar.leap_year)])?;
            csv.write_record(["quarter", &calendar.quarter.to_string()])?;
        }
        TableData::DateArithmetic(arithmetic) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["input", arithmetic.input.as_str()])?;
            csv.write_record(["input_kind", arithmetic.input_kind.as_str()])?;
            csv.write_record(["operation", arithmetic.operation.as_str()])?;
            csv.write_record(["amount", &arithmetic.amount.to_string()])?;
            csv.write_record(["unit", arithmetic.unit.as_str()])?;
            csv.write_record(["result", arithmetic.result.as_str()])?;
            csv.write_record(["result_kind", arithmetic.result_kind.as_str()])?;
            csv.write_record(["month_end_clamped", bool_text(arithmetic.month_end_clamped)])?;
        }
        TableData::DateDiff(diff) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["from", diff.from.as_str()])?;
            csv.write_record(["to", diff.to.as_str()])?;
            csv.write_record(["from_kind", diff.from_kind.as_str()])?;
            csv.write_record(["to_kind", diff.to_kind.as_str()])?;
            csv.write_record(["signed_seconds", &diff.signed_seconds.to_string()])?;
            csv.write_record(["signed_minutes", &diff.signed_minutes.to_string()])?;
            csv.write_record(["signed_hours", &diff.signed_hours.to_string()])?;
            csv.write_record(["signed_days", &diff.signed_days.to_string()])?;
            csv.write_record(["signed_weeks", &diff.signed_weeks.to_string()])?;
            csv.write_record(["direction", &diff.direction.to_string()])?;
            csv.write_record(["absolute_seconds", &diff.absolute_seconds.to_string()])?;
        }
        TableData::DateFormatResult(format) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["input", format.input.as_str()])?;
            csv.write_record(["input_kind", format.input_kind.as_str()])?;
            csv.write_record(["output_format", format.output_format.as_str()])?;
            csv.write_record(["formatted", format.formatted.as_str()])?;
            csv.write_record(["timezone_present", bool_text(format.timezone_present)])?;
        }
        TableData::HolidayCheck(check) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["date", check.date.as_str()])?;
            csv.write_record(["country_code", check.country_code.as_str()])?;
            csv.write_record(["country", check.country.as_str()])?;
            csv.write_record(["is_holiday", bool_text(check.is_holiday)])?;
            csv.write_record([
                "holiday",
                check
                    .holiday
                    .as_ref()
                    .map(|holiday| holiday.name.as_str())
                    .unwrap_or_default(),
            ])?;
            write_coverage_csv(&mut csv, &check.coverage)?;
        }
        TableData::HolidayList(list) => {
            csv.write_record(["date", "country_code", "country", "name"])?;
            for holiday in &list.holidays {
                csv.write_record([
                    holiday.date.as_str(),
                    holiday.country_code.as_str(),
                    holiday.country.as_str(),
                    holiday.name.as_str(),
                ])?;
            }
        }
        TableData::BusinessDayCount(count) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["from", count.from.as_str()])?;
            csv.write_record(["to", count.to.as_str()])?;
            csv.write_record(["inclusive", bool_text(count.inclusive)])?;
            csv.write_record(["direction", &count.direction.to_string()])?;
            csv.write_record(["calendar_days", &count.calendar_days.to_string()])?;
            csv.write_record(["business_days", &count.business_days.to_string()])?;
            csv.write_record(["mode", count.mode.as_str()])?;
            csv.write_record(["skip_holidays", bool_text(count.skip_holidays)])?;
            csv.write_record([
                "country_code",
                count.country_code.as_deref().unwrap_or_default(),
            ])?;
            csv.write_record([
                "holidays_skipped",
                &count.holidays_skipped.len().to_string(),
            ])?;
            if let Some(coverage) = &count.coverage {
                write_coverage_csv(&mut csv, coverage)?;
            }
        }
        TableData::BusinessDaySearch(search) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["input_date", search.input_date.as_str()])?;
            csv.write_record(["business_date", search.business_date.as_str()])?;
            csv.write_record(["direction", search.direction.as_str()])?;
            csv.write_record(["strict", bool_text(search.strict)])?;
            csv.write_record(["days_moved", &search.days_moved.to_string()])?;
            csv.write_record(["mode", search.mode.as_str()])?;
            csv.write_record([
                "country_code",
                search.country_code.as_deref().unwrap_or_default(),
            ])?;
            if let Some(coverage) = &search.coverage {
                write_coverage_csv(&mut csv, coverage)?;
            }
        }
        TableData::TimerRecord(timer) => write_timer_record_csv(&mut csv, timer)?,
        TableData::TimerList(list) => {
            csv.write_record([
                "name",
                "deadline_utc",
                "original_deadline",
                "timezone",
                "description",
                "tags",
                "created_at_utc",
                "updated_at_utc",
                "status",
                "overdue",
                "remaining_seconds",
                "remaining",
            ])?;
            for timer in &list.timers {
                write_timer_row_csv(&mut csv, timer)?;
            }
        }
        TableData::TimerDelete(deleted) => {
            csv.write_record(["field", "value"])?;
            csv.write_record(["name", deleted.name.as_str()])?;
            csv.write_record(["deleted", bool_text(deleted.deleted)])?;
            csv.write_record(["deleted_tags", &deleted.deleted_tags.to_string()])?;
        }
        TableData::TimerCheck(check) => {
            csv.write_record([
                "name",
                "deadline_utc",
                "original_deadline",
                "timezone",
                "description",
                "tags",
                "created_at_utc",
                "updated_at_utc",
                "status",
                "overdue",
                "remaining_seconds",
                "remaining",
            ])?;
            for timer in &check.timers {
                write_timer_row_csv(&mut csv, timer)?;
            }
        }
    }
    csv.flush()?;
    Ok(())
}

fn write_timer_record_csv<W: Write>(csv: &mut csv::Writer<W>, timer: &TimerRecord) -> Result<()> {
    csv.write_record(["field", "value"])?;
    csv.write_record(["name", timer.name.as_str()])?;
    csv.write_record(["deadline_utc", timer.deadline_utc.as_str()])?;
    csv.write_record(["original_deadline", timer.original_deadline.as_str()])?;
    csv.write_record(["timezone", timer.timezone.as_str()])?;
    csv.write_record([
        "description",
        timer.description.as_deref().unwrap_or_default(),
    ])?;
    csv.write_record(["tags", &timer.tags.join(",")])?;
    csv.write_record(["created_at_utc", timer.created_at_utc.as_str()])?;
    csv.write_record(["updated_at_utc", timer.updated_at_utc.as_str()])?;
    csv.write_record(["status", timer.status.as_str()])?;
    csv.write_record(["overdue", bool_text(timer.overdue)])?;
    csv.write_record(["remaining_seconds", &timer.remaining_seconds.to_string()])?;
    csv.write_record(["remaining", timer.remaining.as_str()])?;
    Ok(())
}

fn write_timer_row_csv<W: Write>(csv: &mut csv::Writer<W>, timer: &TimerRecord) -> Result<()> {
    csv.write_record([
        timer.name.as_str(),
        timer.deadline_utc.as_str(),
        timer.original_deadline.as_str(),
        timer.timezone.as_str(),
        timer.description.as_deref().unwrap_or_default(),
        &timer.tags.join(","),
        timer.created_at_utc.as_str(),
        timer.updated_at_utc.as_str(),
        timer.status.as_str(),
        bool_text(timer.overdue),
        &timer.remaining_seconds.to_string(),
        timer.remaining.as_str(),
    ])?;
    Ok(())
}

fn write_coverage_csv<W: Write>(
    csv: &mut csv::Writer<W>,
    coverage: &crate::models::HolidayCoverage,
) -> Result<()> {
    csv.write_record(["coverage_source", coverage.source.as_str()])?;
    csv.write_record(["coverage_start_year", &coverage.start_year.to_string()])?;
    csv.write_record(["coverage_end_year", &coverage.end_year.to_string()])?;
    csv.write_record(["runtime_network", bool_text(coverage.runtime_network)])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_paths() -> ConfigPaths {
        ConfigPaths {
            config_path: "/tmp/time-keep/config.toml".to_string(),
            data_dir: "/tmp/time-keep/data".to_string(),
            timer_db_path: "/tmp/time-keep/data/timers.db".to_string(),
        }
    }

    #[test]
    fn renders_config_paths_as_json() {
        let paths = sample_paths();
        let mut output = Vec::new();
        render_to_writer(
            OutputFormat::Json,
            &paths,
            TableData::ConfigPaths(&paths),
            &mut output,
        )
        .expect("render succeeds");
        let text = String::from_utf8(output).expect("utf8 output");
        assert!(text.contains("\"config_path\""));
        assert!(text.contains("\"timer_db_path\""));
    }

    #[test]
    fn renders_config_paths_as_csv() {
        let paths = sample_paths();
        let mut output = Vec::new();
        render_to_writer(
            OutputFormat::Csv,
            &paths,
            TableData::ConfigPaths(&paths),
            &mut output,
        )
        .expect("render succeeds");
        let text = String::from_utf8(output).expect("utf8 output");
        assert!(text.contains("field,value"));
        assert!(text.contains("timer_db_path,/tmp/time-keep/data/timers.db"));
    }

    #[test]
    fn renders_error_as_structured_json() {
        let err = TimeKeepError::invalid_params("invalid timezone");
        let mut output = Vec::new();
        render_error_to_writer(OutputFormat::Json, &err, &mut output).expect("render succeeds");
        let text = String::from_utf8(output).expect("utf8 output");
        assert!(text.contains("\"error_code\""));
        assert!(text.contains("\"INVALID_PARAMS\""));
    }
}
