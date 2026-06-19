use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    path::Path,
    thread,
    time::{Duration, Instant},
};

use chrono::{DateTime, FixedOffset, NaiveDateTime, SecondsFormat, Utc};
use rusqlite::{
    Connection, OptionalExtension, Transaction, TransactionBehavior,
    ffi::ErrorCode as SqliteErrorCode, params,
};
use serde_json::json;

use crate::{
    error::{Result, TimeKeepError},
    models::{TimerCheck, TimerDelete, TimerList, TimerRecord},
};

const SCHEMA_VERSION: i64 = 1;
const BUSY_TIMEOUT_MS: u64 = 5_000;
const BUSY_RETRY_DELAY_MS: u64 = 25;
const DEADLINE_NAIVE_PATTERN: &str = "%Y-%m-%dT%H:%M:%S";
const DEADLINE_NAIVE_FRACTIONAL_PATTERN: &str = "%Y-%m-%dT%H:%M:%S%.f";

pub(crate) struct TimerStore {
    conn: Connection,
}

impl TimerStore {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        prepare_database_path(path)?;
        let mut conn = Connection::open(path)?;
        configure_connection(&conn)?;
        run_migrations(&mut conn)?;
        Ok(Self { conn })
    }

    pub(crate) fn set_timer(
        &mut self,
        name: &str,
        deadline: &str,
        description: Option<&str>,
        tags: &[String],
    ) -> Result<TimerRecord> {
        let name = normalize_timer_name(name)?;
        let parsed_deadline = parse_deadline(deadline)?;
        let parsed_deadline_utc = parsed_deadline.with_timezone(&Utc);
        let deadline_epoch_nanos = epoch_nanos(parsed_deadline_utc)?;
        let deadline_utc = format_utc(parsed_deadline_utc);
        let timezone = parsed_deadline.offset().to_string();
        let description = normalize_description(description);
        let tags = normalize_tags(tags)?;
        let now = Utc::now();
        let now_utc = format_utc(now);

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO timers (
                name,
                deadline_utc,
                deadline_epoch_nanos,
                original_deadline,
                timezone,
                description,
                created_at_utc,
                updated_at_utc
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
            ON CONFLICT(name) DO UPDATE SET
                deadline_utc = excluded.deadline_utc,
                deadline_epoch_nanos = excluded.deadline_epoch_nanos,
                original_deadline = excluded.original_deadline,
                timezone = excluded.timezone,
                description = excluded.description,
                updated_at_utc = excluded.updated_at_utc",
            params![
                name,
                deadline_utc,
                deadline_epoch_nanos,
                deadline,
                timezone,
                description,
                now_utc,
            ],
        )?;
        let timer_id = timer_id(&tx, &name)?;
        replace_tags(&tx, timer_id, &tags)?;
        prune_unused_tags(&tx)?;
        let record = row_to_record(&tx, timer_row_by_name(&tx, &name)?, Utc::now())?;
        tx.commit()?;

        Ok(record)
    }

    pub(crate) fn get_timer(&self, name: &str) -> Result<TimerRecord> {
        let name = normalize_timer_name(name)?;
        let tx = self.conn.unchecked_transaction()?;
        let record = row_to_record(&tx, timer_row_by_name(&tx, &name)?, Utc::now())?;
        tx.commit()?;
        Ok(record)
    }

    pub(crate) fn list_timers(&self, tag: Option<&str>) -> Result<TimerList> {
        let tag = tag.map(normalize_tag).transpose()?;
        let tx = self.conn.unchecked_transaction()?;
        let rows = if let Some(tag) = &tag {
            rows_for_tag(&tx, tag)?
        } else {
            all_rows(&tx)?
        };
        let now = Utc::now();
        let timers = rows
            .into_iter()
            .map(|row| row_to_record(&tx, row, now))
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(TimerList {
            tag,
            count: timers.len(),
            timers,
        })
    }

    pub(crate) fn delete_timer(&mut self, name: &str) -> Result<TimerDelete> {
        let name = normalize_timer_name(name)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let timer_id = tx
            .query_row(
                "SELECT id FROM timers WHERE name = ?1",
                params![name],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .ok_or_else(|| timer_not_found_error(&name))?;
        let deleted_tags = tx.query_row(
            "SELECT COUNT(*) FROM timer_tags WHERE timer_id = ?1",
            params![timer_id],
            |row| row.get::<_, i64>(0),
        )?;
        tx.execute("DELETE FROM timers WHERE id = ?1", params![timer_id])?;
        prune_unused_tags(&tx)?;
        tx.commit()?;

        Ok(TimerDelete {
            name,
            deleted: true,
            deleted_tags: usize::try_from(deleted_tags).map_err(|_| {
                TimeKeepError::new(
                    crate::error::ErrorCode::Internal,
                    "timer tag count overflowed supported range",
                )
            })?,
        })
    }

    pub(crate) fn check_timers(&self) -> Result<TimerCheck> {
        let now = Utc::now();
        let generated_at_utc = format_utc(now);
        let tx = self.conn.unchecked_transaction()?;
        let timers = all_rows(&tx)?
            .into_iter()
            .map(|row| row_to_record(&tx, row, now))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|timer| timer.overdue)
            .collect::<Vec<_>>();
        tx.commit()?;

        Ok(TimerCheck {
            generated_at_utc,
            count: timers.len(),
            timers,
        })
    }
}

fn row_to_record(conn: &Connection, row: TimerRow, now: DateTime<Utc>) -> Result<TimerRecord> {
    let tags = tags_for_timer(conn, row.id)?;
    let deadline = parse_stored_utc(&row.deadline_utc)?;
    let overdue = deadline < now;
    let mut remaining_seconds = deadline.signed_duration_since(now).num_seconds();
    if overdue && remaining_seconds == 0 {
        remaining_seconds = -1;
    }
    let status = if overdue { "overdue" } else { "pending" }.to_string();
    let remaining = human_remaining(remaining_seconds);

    Ok(TimerRecord {
        name: row.name,
        deadline_utc: row.deadline_utc,
        original_deadline: row.original_deadline,
        timezone: row.timezone,
        description: row.description,
        tags,
        created_at_utc: row.created_at_utc,
        updated_at_utc: row.updated_at_utc,
        status,
        overdue,
        remaining_seconds,
        remaining,
    })
}

fn timer_row_by_name(conn: &Connection, name: &str) -> Result<TimerRow> {
    conn.query_row(
        "SELECT id, name, deadline_utc, original_deadline, timezone, description,
            created_at_utc, updated_at_utc
        FROM timers
        WHERE name = ?1",
        params![name],
        TimerRow::from_row,
    )
    .optional()?
    .ok_or_else(|| timer_not_found_error(name))
}

fn all_rows(conn: &Connection) -> Result<Vec<TimerRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, deadline_utc, original_deadline, timezone, description,
            created_at_utc, updated_at_utc
        FROM timers
        ORDER BY deadline_epoch_nanos ASC, name ASC",
    )?;
    let rows = stmt
        .query_map([], TimerRow::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

fn rows_for_tag(conn: &Connection, tag: &str) -> Result<Vec<TimerRow>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.deadline_utc, t.original_deadline, t.timezone,
            t.description, t.created_at_utc, t.updated_at_utc
        FROM timers t
        INNER JOIN timer_tags tt ON tt.timer_id = t.id
        INNER JOIN tags g ON g.id = tt.tag_id
        WHERE g.name = ?1
        ORDER BY t.deadline_epoch_nanos ASC, t.name ASC",
    )?;
    let rows = stmt
        .query_map(params![tag], TimerRow::from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[derive(Debug)]
struct TimerRow {
    id: i64,
    name: String,
    deadline_utc: String,
    original_deadline: String,
    timezone: String,
    description: Option<String>,
    created_at_utc: String,
    updated_at_utc: String,
}

impl TimerRow {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            deadline_utc: row.get(2)?,
            original_deadline: row.get(3)?,
            timezone: row.get(4)?,
            description: row.get(5)?,
            created_at_utc: row.get(6)?,
            updated_at_utc: row.get(7)?,
        })
    }
}

fn prepare_database_path(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        let parent_existed = parent.exists();
        fs::create_dir_all(parent)?;
        if !parent_existed {
            set_private_dir_permissions(parent)?;
        }
    }

    let file = open_private_database_file(path)?;
    set_private_file_permissions(&file)?;
    Ok(())
}

fn open_private_database_file(path: &Path) -> Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    set_private_file_create_mode(&mut options);
    Ok(options.open(path)?)
}

#[cfg(unix)]
fn set_private_file_create_mode(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_private_file_create_mode(_options: &mut OpenOptions) {}

fn configure_connection(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_millis(BUSY_TIMEOUT_MS))?;
    retry_on_sqlite_lock(|| {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA synchronous = NORMAL;",
        )
    })?;
    let journal_mode: String =
        retry_on_sqlite_lock(|| conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0)))?;
    verify_wal_journal_mode(&journal_mode)?;
    Ok(())
}

fn retry_on_sqlite_lock<T>(mut operation: impl FnMut() -> rusqlite::Result<T>) -> Result<T> {
    let deadline = Instant::now() + Duration::from_millis(BUSY_TIMEOUT_MS);
    loop {
        match operation() {
            Ok(value) => return Ok(value),
            Err(err) if is_sqlite_lock_error(&err) && Instant::now() < deadline => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                thread::sleep(remaining.min(Duration::from_millis(BUSY_RETRY_DELAY_MS)));
            }
            Err(err) => return Err(err.into()),
        }
    }
}

fn is_sqlite_lock_error(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(code, _)
            if matches!(code.code, SqliteErrorCode::DatabaseBusy | SqliteErrorCode::DatabaseLocked)
    )
}

fn run_migrations(conn: &mut Connection) -> Result<()> {
    let version: i64 =
        retry_on_sqlite_lock(|| conn.pragma_query_value(None, "user_version", |row| row.get(0)))?;
    if version > SCHEMA_VERSION {
        return Err(TimeKeepError::new(
            crate::error::ErrorCode::Internal,
            format!("timer database schema {version} is newer than supported {SCHEMA_VERSION}"),
        ));
    }
    if version < 1 {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(TimeKeepError::new(
                crate::error::ErrorCode::Internal,
                format!("timer database schema {version} is newer than supported {SCHEMA_VERSION}"),
            ));
        }
        if version < 1 {
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS timers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    deadline_utc TEXT NOT NULL,
                    deadline_epoch_nanos INTEGER NOT NULL,
                    original_deadline TEXT NOT NULL,
                    timezone TEXT NOT NULL,
                    description TEXT,
                    created_at_utc TEXT NOT NULL,
                    updated_at_utc TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS tags (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE
                );

                CREATE TABLE IF NOT EXISTS timer_tags (
                    timer_id INTEGER NOT NULL REFERENCES timers(id) ON DELETE CASCADE,
                    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                    PRIMARY KEY (timer_id, tag_id)
                );

                CREATE INDEX IF NOT EXISTS idx_timers_deadline_epoch_nanos
                    ON timers(deadline_epoch_nanos);
                CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name);
                CREATE INDEX IF NOT EXISTS idx_timer_tags_tag_id ON timer_tags(tag_id);
                PRAGMA user_version = 1;",
            )?;
        }
        tx.commit()?;
    }
    Ok(())
}

fn verify_wal_journal_mode(journal_mode: &str) -> Result<()> {
    if journal_mode.eq_ignore_ascii_case("wal") {
        return Ok(());
    }

    Err(TimeKeepError::new(
        crate::error::ErrorCode::Internal,
        format!("failed to enable SQLite WAL journal mode: SQLite returned {journal_mode}"),
    )
    .with_detail("journal_mode", json!(journal_mode)))
}
fn timer_id(tx: &Transaction<'_>, name: &str) -> Result<i64> {
    Ok(tx.query_row(
        "SELECT id FROM timers WHERE name = ?1",
        params![name],
        |row| row.get(0),
    )?)
}

fn replace_tags(tx: &Transaction<'_>, timer_id: i64, tags: &[String]) -> Result<()> {
    tx.execute(
        "DELETE FROM timer_tags WHERE timer_id = ?1",
        params![timer_id],
    )?;
    for tag in tags {
        tx.execute(
            "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
            params![tag],
        )?;
        let tag_id: i64 =
            tx.query_row("SELECT id FROM tags WHERE name = ?1", params![tag], |row| {
                row.get(0)
            })?;
        tx.execute(
            "INSERT OR IGNORE INTO timer_tags (timer_id, tag_id) VALUES (?1, ?2)",
            params![timer_id, tag_id],
        )?;
    }
    Ok(())
}

fn prune_unused_tags(tx: &Transaction<'_>) -> Result<()> {
    tx.execute(
        "DELETE FROM tags
        WHERE NOT EXISTS (
            SELECT 1 FROM timer_tags WHERE timer_tags.tag_id = tags.id
        )",
        [],
    )?;
    Ok(())
}

fn tags_for_timer(conn: &Connection, timer_id: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT g.name
        FROM tags g
        INNER JOIN timer_tags tt ON tt.tag_id = g.id
        WHERE tt.timer_id = ?1
        ORDER BY g.name ASC",
    )?;
    let tags = stmt
        .query_map(params![timer_id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tags)
}

fn normalize_timer_name(input: &str) -> Result<String> {
    let name = input.trim();
    if name.is_empty() {
        Err(TimeKeepError::invalid_params("timer name cannot be empty")
            .with_detail("parameter", json!("name")))
    } else {
        Ok(name.to_string())
    }
}

fn normalize_description(input: Option<&str>) -> Option<String> {
    input
        .map(str::trim)
        .filter(|description| !description.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_tags(tags: &[String]) -> Result<Vec<String>> {
    let mut normalized = BTreeSet::new();
    for tag in tags {
        normalized.insert(normalize_tag(tag)?);
    }
    Ok(normalized.into_iter().collect())
}

fn normalize_tag(input: &str) -> Result<String> {
    let tag = input.trim().to_lowercase();
    if tag.is_empty() {
        Err(TimeKeepError::invalid_params("timer tag cannot be empty")
            .with_detail("parameter", json!("tag")))
    } else {
        Ok(tag)
    }
}

fn parse_deadline(input: &str) -> Result<DateTime<FixedOffset>> {
    if let Ok(datetime) = DateTime::parse_from_rfc3339(input) {
        return Ok(datetime);
    }

    for pattern in [DEADLINE_NAIVE_PATTERN, DEADLINE_NAIVE_FRACTIONAL_PATTERN] {
        if let Ok(datetime) = NaiveDateTime::parse_from_str(input, pattern) {
            return Ok(DateTime::from_naive_utc_and_offset(
                datetime,
                utc_fixed_offset(),
            ));
        }
    }

    Err(
        TimeKeepError::invalid_params(format!("invalid ISO 8601/RFC3339 deadline: {input}"))
            .with_detail("parameter", json!("deadline"))
            .with_detail("value", json!(input)),
    )
}

fn parse_stored_utc(input: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(input)
        .map(|datetime| datetime.with_timezone(&Utc))
        .map_err(|_| {
            TimeKeepError::new(
                crate::error::ErrorCode::Internal,
                format!("stored timer deadline is invalid: {input}"),
            )
        })
}

fn format_utc(datetime: DateTime<Utc>) -> String {
    datetime.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

fn epoch_nanos(datetime: DateTime<Utc>) -> Result<i64> {
    datetime.timestamp_nanos_opt().ok_or_else(|| {
        TimeKeepError::invalid_params("deadline is outside the supported timestamp range")
            .with_detail("parameter", json!("deadline"))
    })
}

fn utc_fixed_offset() -> FixedOffset {
    FixedOffset::east_opt(0).expect("zero offset is valid")
}

fn human_remaining(seconds: i64) -> String {
    if seconds == 0 {
        return "due now".to_string();
    }

    let duration = human_duration(seconds.unsigned_abs());
    if seconds < 0 {
        format!("overdue by {duration}")
    } else {
        format!("{duration} remaining")
    }
}

fn human_duration(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let secs = seconds % 60;
    let mut parts = Vec::new();

    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 && parts.len() < 2 {
        parts.push(format!("{minutes}m"));
    }
    if parts.is_empty() {
        parts.push(format!("{secs}s"));
    }

    parts.join(" ")
}

fn timer_not_found_error(name: &str) -> TimeKeepError {
    TimeKeepError::invalid_params(format!("timer not found: {name}"))
        .with_detail("parameter", json!("name"))
        .with_detail("value", json!(name))
}

#[cfg(unix)]
fn set_private_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(file: &fs::File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_file: &fs::File) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        path::PathBuf,
        sync::{Arc, Barrier},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn set_get_persists_across_store_instances() {
        let path = temp_db_path("persist");
        let mut store = TimerStore::open(&path).expect("open store");
        let timer = store
            .set_timer(
                "q3-planning",
                "2026-07-01T17:00:00-04:00",
                Some("Q3 planning due"),
                &[
                    "Work".to_string(),
                    "planning".to_string(),
                    "work".to_string(),
                ],
            )
            .expect("set timer");
        assert_eq!(timer.deadline_utc, "2026-07-01T21:00:00Z");
        assert_eq!(timer.original_deadline, "2026-07-01T17:00:00-04:00");
        assert_eq!(timer.timezone, "-04:00");
        assert_eq!(timer.tags, ["planning", "work"]);
        drop(store);

        let store = TimerStore::open(&path).expect("reopen store");
        let timer = store.get_timer("q3-planning").expect("get timer");
        assert_eq!(timer.description.as_deref(), Some("Q3 planning due"));
        assert_eq!(timer.deadline_utc, "2026-07-01T21:00:00Z");
    }

    #[test]
    fn naive_deadline_defaults_to_utc() {
        let path = temp_db_path("naive-deadline");
        let mut store = TimerStore::open(&path).expect("open store");
        let timer = store
            .set_timer("standup", "2026-07-01T17:00:00.500", None, &[])
            .expect("set timer");

        assert_eq!(timer.deadline_utc, "2026-07-01T17:00:00.500Z");
        assert_eq!(timer.original_deadline, "2026-07-01T17:00:00.500");
        assert_eq!(timer.timezone, "+00:00");
    }

    #[test]
    fn wal_journal_mode_is_required() {
        assert!(verify_wal_journal_mode("wal").is_ok());
        assert!(verify_wal_journal_mode("WAL").is_ok());
        let err = verify_wal_journal_mode("delete").expect_err("wal required");
        assert_eq!(err.code().as_str(), "INTERNAL_ERROR");
        assert_eq!(err.details().get("journal_mode"), Some(&json!("delete")));
    }

    #[test]
    fn tag_filtering_is_case_normalized_and_sorted() {
        let path = temp_db_path("tags");
        let mut store = TimerStore::open(&path).expect("open store");
        store
            .set_timer(
                "alpha",
                "2026-07-01T17:00:00Z",
                None,
                &["Work".to_string(), "alpha".to_string()],
            )
            .expect("set alpha");
        store
            .set_timer(
                "beta",
                "2026-07-02T17:00:00Z",
                None,
                &["Personal".to_string()],
            )
            .expect("set beta");

        let list = store.list_timers(Some("WORK")).expect("list work");
        assert_eq!(list.tag.as_deref(), Some("work"));
        assert_eq!(list.count, 1);
        assert_eq!(list.timers[0].name, "alpha");
        assert_eq!(list.timers[0].tags, ["alpha", "work"]);
    }

    #[test]
    fn unicode_tag_filtering_is_case_normalized() {
        let path = temp_db_path("unicode-tags");
        let mut store = TimerStore::open(&path).expect("open store");
        store
            .set_timer(
                "international",
                "2026-07-01T17:00:00Z",
                None,
                &["Équipe".to_string()],
            )
            .expect("set timer");

        let list = store.list_timers(Some("équipe")).expect("list tag");
        assert_eq!(list.count, 1);
        assert_eq!(list.timers[0].name, "international");
        assert_eq!(list.timers[0].tags, ["équipe"]);
    }

    #[test]
    fn delete_removes_timer_tags() {
        let path = temp_db_path("delete");
        let mut store = TimerStore::open(&path).expect("open store");
        store
            .set_timer(
                "delete-me",
                "2026-07-01T17:00:00Z",
                None,
                &["work".to_string()],
            )
            .expect("set timer");

        let deleted = store.delete_timer("delete-me").expect("delete timer");
        assert!(deleted.deleted);
        assert_eq!(deleted.deleted_tags, 1);
        assert!(store.get_timer("delete-me").is_err());
        assert_eq!(store.list_timers(Some("work")).expect("list").count, 0);
    }

    #[test]
    fn delete_waits_for_existing_write_lock() {
        let path = temp_db_path("delete-write-lock");
        let mut store = TimerStore::open(&path).expect("open store");
        store
            .set_timer("delete-me", "2026-07-01T17:00:00Z", None, &[])
            .expect("set timer");
        drop(store);

        let mut writer = TimerStore::open(&path).expect("open writer");
        let tx = writer
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("start writer tx");
        tx.execute(
            "UPDATE timers SET description = ?1 WHERE name = ?2",
            params!["writer holds lock", "delete-me"],
        )
        .expect("update timer");

        let handle = {
            let path = path.clone();
            thread::spawn(move || {
                let mut deleter = TimerStore::open(&path).expect("open deleter");
                deleter.delete_timer("delete-me")
            })
        };
        thread::sleep(Duration::from_millis(100));
        tx.commit().expect("commit writer");

        let deleted = handle
            .join()
            .expect("delete thread joins")
            .expect("delete waits for write lock");
        assert!(deleted.deleted);
    }

    #[test]
    fn check_returns_only_overdue_timers() {
        let path = temp_db_path("check");
        let mut store = TimerStore::open(&path).expect("open store");
        let now = Utc::now();
        let past = format_utc(now - chrono::TimeDelta::days(1));
        let future = format_utc(now + chrono::TimeDelta::days(365));
        store.set_timer("past", &past, None, &[]).expect("set past");
        store
            .set_timer("future", &future, None, &[])
            .expect("set future");

        let check = store.check_timers().expect("check timers");
        assert_eq!(check.count, 1);
        assert_eq!(check.timers[0].name, "past");
        assert_eq!(check.timers[0].status, "overdue");
    }

    #[test]
    fn subsecond_overdue_deadline_is_reported_overdue() {
        let path = temp_db_path("subsecond-overdue");
        let mut store = TimerStore::open(&path).expect("open store");
        let now = Utc::now();
        let deadline = now - chrono::TimeDelta::milliseconds(500);
        store
            .set_timer("subsecond", &format_utc(deadline), None, &[])
            .expect("set subsecond timer");

        let row = all_rows(&store.conn).expect("load rows").remove(0);
        let timer = row_to_record(&store.conn, row, now).expect("timer record");
        assert!(timer.overdue);
        assert_eq!(timer.status, "overdue");
        assert!(timer.remaining_seconds < 0);
    }

    #[test]
    fn timer_records_use_one_read_snapshot_for_rows_and_tags() {
        let path = temp_db_path("read-snapshot");
        let mut reader = TimerStore::open(&path).expect("open reader");
        reader
            .set_timer(
                "snapshot",
                "2026-07-01T17:00:00Z",
                None,
                &["old".to_string()],
            )
            .expect("set initial timer");

        let tx = reader.conn.unchecked_transaction().expect("start read tx");
        let row = timer_row_by_name(&tx, "snapshot").expect("read timer row");

        let mut writer = TimerStore::open(&path).expect("open writer");
        writer
            .set_timer(
                "snapshot",
                "2026-07-02T17:00:00Z",
                None,
                &["new".to_string()],
            )
            .expect("update timer");

        let record = row_to_record(&tx, row, Utc::now()).expect("read timer tags");
        assert_eq!(record.deadline_utc, "2026-07-01T17:00:00Z");
        assert_eq!(record.tags, ["old"]);
        tx.commit().expect("commit read tx");
    }

    #[cfg(unix)]
    #[test]
    fn existing_data_dir_permissions_are_preserved() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_db_path("existing-dir-permissions");
        let parent = path.parent().expect("timer database parent").to_path_buf();
        fs::create_dir_all(&parent).expect("create parent dir");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755))
            .expect("set parent permissions");

        TimerStore::open(&path).expect("open store");

        let mode = fs::metadata(&parent)
            .expect("parent metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o755);
    }

    #[cfg(unix)]
    #[test]
    fn database_file_is_created_private() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_db_path("file-permissions");

        TimerStore::open(&path).expect("open store");

        let mode = fs::metadata(&path)
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn rapid_independent_writes_do_not_corrupt_database() {
        let path = temp_db_path("rapid");
        TimerStore::open(&path).expect("open store");
        let handles = (0..8)
            .map(|idx| {
                let path = path.clone();
                thread::spawn(move || {
                    let mut store = TimerStore::open(&path).expect("open store in thread");
                    store
                        .set_timer(
                            &format!("timer-{idx}"),
                            &format!("2026-07-{:02}T17:00:00Z", idx + 1),
                            None,
                            &["batch".to_string()],
                        )
                        .expect("set timer");
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("thread joins");
        }

        let store = TimerStore::open(&path).expect("reopen store");
        assert_eq!(store.list_timers(Some("batch")).expect("list").count, 8);
    }

    #[test]
    fn concurrent_first_run_initialization_is_idempotent() {
        let path = temp_db_path("first-run");
        let barrier = Arc::new(Barrier::new(6));
        let handles = (0..6)
            .map(|idx| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let mut store = TimerStore::open(&path).expect("open empty store in thread");
                    store
                        .set_timer(
                            &format!("first-run-{idx}"),
                            &format!("2026-08-{:02}T17:00:00Z", idx + 1),
                            None,
                            &["first-run".to_string()],
                        )
                        .expect("set timer");
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            handle.join().expect("thread joins");
        }

        let store = TimerStore::open(&path).expect("reopen store");
        assert_eq!(
            store
                .list_timers(Some("first-run"))
                .expect("list first-run")
                .count,
            6
        );
    }

    #[test]
    fn fractional_deadlines_sort_chronologically() {
        let path = temp_db_path("fractional-order");
        let mut store = TimerStore::open(&path).expect("open store");
        store
            .set_timer("whole", "2026-07-01T17:00:00Z", None, &[])
            .expect("set whole");
        store
            .set_timer("half", "2026-07-01T17:00:00.500Z", None, &[])
            .expect("set half");

        let list = store.list_timers(None).expect("list timers");
        assert_eq!(list.timers[0].name, "whole");
        assert_eq!(list.timers[1].name, "half");
    }

    #[test]
    fn invalid_deadline_is_invalid_params() {
        let path = temp_db_path("invalid");
        let mut store = TimerStore::open(&path).expect("open store");
        let err = store
            .set_timer("bad", "2026-07-01 17:00:00", None, &[])
            .expect_err("invalid deadline");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
        assert_eq!(err.details().get("parameter"), Some(&json!("deadline")));
    }

    #[test]
    fn empty_tag_is_invalid_params() {
        let path = temp_db_path("empty-tag");
        let mut store = TimerStore::open(&path).expect("open store");
        let err = store
            .set_timer("bad-tag", "2026-07-01T17:00:00Z", None, &[" ".to_string()])
            .expect_err("invalid tag");
        assert_eq!(err.code().as_str(), "INVALID_PARAMS");
        assert_eq!(err.details().get("parameter"), Some(&json!("tag")));
    }

    fn temp_db_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        env::temp_dir()
            .join(format!(
                "time-keep-db-test-{name}-{}-{nanos}",
                std::process::id()
            ))
            .join("timers.db")
    }
}
