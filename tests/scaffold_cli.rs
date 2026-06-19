use std::{
    env, fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_time-keep")
}

#[test]
fn help_lists_planned_command_families() {
    let output = Command::new(binary())
        .arg("--help")
        .output()
        .expect("run help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("help is utf8");
    for command in [
        "now",
        "tz",
        "convert",
        "format",
        "calc",
        "calendar",
        "biz",
        "holiday",
        "timer",
        "config",
        "server",
        "completions",
    ] {
        assert!(stdout.contains(command), "help should include {command}");
    }
}

#[test]
fn config_path_outputs_json_by_default() {
    let output = Command::new(binary())
        .args(["config", "path"])
        .output()
        .expect("run config path");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert!(json.get("config_path").is_some());
    assert!(json.get("data_dir").is_some());
    assert!(json.get("timer_db_path").is_some());
}

#[test]
fn invalid_xdg_values_fall_back_to_home_paths() {
    let output = Command::new(binary())
        .args(["config", "path"])
        .env("HOME", "/tmp/time-keep-home")
        .env("XDG_CONFIG_HOME", "relative")
        .env("XDG_DATA_HOME", "")
        .output()
        .expect("run config path");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(
        json["config_path"],
        "/tmp/time-keep-home/.config/time-keep/config.toml"
    );
    assert_eq!(
        json["data_dir"],
        "/tmp/time-keep-home/.local/share/time-keep"
    );
}

#[test]
fn config_path_table_output_uses_field_value_contract() {
    let home = temp_data_dir("table-home");
    let output = Command::new(binary())
        .args(["--table", "config", "path"])
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join("xdg-config"))
        .env("XDG_DATA_HOME", home.join("xdg-data"))
        .output()
        .expect("run config path table output");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("table output is utf8");
    assert!(stdout.contains("Field"));
    assert!(stdout.contains("Value"));
    assert!(stdout.contains("config_path"));
    assert!(stdout.contains("timer_db_path"));
    assert!(!stdout.trim_start().starts_with('{'));
}

#[test]
fn calendar_csv_output_has_stable_field_value_rows() {
    let output = Command::new(binary())
        .args(["--output", "csv", "calendar", "2026-06-18"])
        .output()
        .expect("run calendar csv output");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("csv output is utf8");
    assert!(stdout.starts_with("field,value\n"));
    assert!(stdout.contains("date,2026-06-18\n"));
    assert!(stdout.contains("weekday,Thursday\n"));
    assert!(stdout.contains("quarter,2\n"));
}

#[test]
fn csv_error_output_uses_stable_error_columns() {
    let output = Command::new(binary())
        .args(["--output", "csv", "tz", "info", "Madrid"])
        .output()
        .expect("run invalid timezone csv output");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("csv error is utf8");
    assert!(stderr.starts_with("error_code,message\n"));
    assert!(stderr.contains("INVALID_PARAMS,invalid IANA timezone name: Madrid\n"));
}

#[test]
fn server_stdio_initializes_and_lists_tools() {
    let data_dir = temp_data_dir("server-stdio");
    let mut child = Command::new(binary())
        .args(["server", "start", "--transport", "stdio"])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn stdio server");

    let mut stdin = child.stdin.take().expect("server stdin");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-03-26","capabilities":{{}},"clientInfo":{{"name":"time-keep-test","version":"0"}}}}}}"#
    )
    .expect("write initialize");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#
    )
    .expect("write tools/list");
    drop(stdin);

    let output = child.wait_with_output().expect("wait for stdio server");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    let responses = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON-RPC line"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["result"]["protocolVersion"], "2025-03-26");
    assert!(responses[0]["result"]["capabilities"]["tools"].is_object());
    let tools = responses[1]["result"]["tools"]
        .as_array()
        .expect("tools array");
    assert_eq!(tools.len(), 15);
    assert_eq!(tools[0]["name"], "current_time");
}

#[test]
fn server_http_serves_health_ready_and_tools_list() {
    let port = unused_port();
    let data_dir = temp_data_dir("server-http");
    let mut child = ChildGuard::new(
        Command::new(binary())
            .args([
                "server",
                "start",
                "--transport",
                "streamable-http",
                "--http-port",
                &port.to_string(),
            ])
            .env("TIME_KEEP_DATA_DIR", &data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn HTTP server"),
    );

    poll_http_ok(port, "/healthz");
    let ready = http_request(port, "GET /readyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
    assert!(ready.starts_with("HTTP/1.1 200 OK"));
    assert!(ready.ends_with("ready\n"));

    let body = r#"{"jsonrpc":"2.0","id":"tools","method":"tools/list","params":{}}"#;
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let response = http_request(port, &request);
    assert!(response.starts_with("HTTP/1.1 200 OK"));
    let (_, response_body) = response.split_once("\r\n\r\n").expect("HTTP body");
    let json: serde_json::Value = serde_json::from_str(response_body).expect("JSON-RPC body");
    let tools = json["result"]["tools"].as_array().expect("tools array");
    assert_eq!(tools.len(), 15);
    assert!(tools.iter().any(|tool| tool["name"] == "timer_check"));

    let mut invalid_utf8_request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: 1\r\n\r\n"
    )
    .into_bytes();
    invalid_utf8_request.push(0xff);
    let invalid_utf8_response = http_request_bytes(port, &invalid_utf8_request);
    assert!(
        invalid_utf8_response.starts_with("HTTP/1.1 400 Bad Request"),
        "unexpected invalid-UTF-8 response: {invalid_utf8_response:?}"
    );

    let large_header = "a".repeat(20_000);
    let rejected = http_request(
        port,
        &format!("GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Fill: {large_header}\r\n\r\n"),
    );
    assert!(
        rejected.starts_with("HTTP/1.1 400 Bad Request"),
        "unexpected oversized-header response: {rejected:?}"
    );

    child.kill_and_wait();
}

#[test]
fn server_http_mcp_timer_tool_calls_persist_state() {
    let port = unused_port();
    let data_dir = temp_data_dir("server-http-timer");
    let mut child = ChildGuard::new(
        Command::new(binary())
            .args([
                "server",
                "start",
                "--transport",
                "streamable-http",
                "--http-port",
                &port.to_string(),
            ])
            .env("TIME_KEEP_DATA_DIR", &data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn HTTP server"),
    );

    poll_http_ok(port, "/healthz");

    let set = mcp_http_post_json(
        port,
        r#"{"jsonrpc":"2.0","id":"set","method":"tools/call","params":{"name":"timer_set","arguments":{"name":"deploy-window","deadline":"2026-07-01T17:00:00-04:00","description":"Deploy window","tags":["Ops","Release"]}}}"#,
    );
    assert_eq!(set["result"]["isError"], false);
    let payload = mcp_text_payload(&set);
    assert_eq!(payload["name"], "deploy-window");
    assert_eq!(payload["deadline_utc"], "2026-07-01T21:00:00Z");
    assert_eq!(payload["tags"][0], "ops");
    assert_eq!(payload["tags"][1], "release");

    let list = mcp_http_post_json(
        port,
        r#"{"jsonrpc":"2.0","id":"list","method":"tools/call","params":{"name":"timer_list","arguments":{"tag":"OPS"}}}"#,
    );
    let payload = mcp_text_payload(&list);
    assert_eq!(payload["tag"], "ops");
    assert_eq!(payload["count"], 1);
    assert_eq!(payload["timers"][0]["name"], "deploy-window");

    child.kill_and_wait();
}

#[test]
fn documented_singular_date_unit_reaches_scaffold_handler() {
    let output = Command::new(binary())
        .args(["calc", "add", "2026-01-31", "1", "month"])
        .output()
        .expect("run documented calc example");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["result"], "2026-02-28");
    assert_eq!(json["unit"], "months");
    assert_eq!(json["month_end_clamped"], true);
}

#[test]
fn completions_zsh_smoke() {
    let output = Command::new(binary())
        .args(["completions", "zsh"])
        .output()
        .expect("run completions");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("completion is utf8");
    assert!(stdout.contains("_time-keep"));
}

#[test]
fn now_outputs_requested_timezones() {
    let output = Command::new(binary())
        .args(["now", "--tz", "UTC", "--tz", "Europe/Madrid"])
        .output()
        .expect("run now");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    let times = json["times"].as_array().expect("times array");
    assert_eq!(times.len(), 2);
    assert_eq!(times[0]["timezone"], "UTC");
    assert_eq!(times[1]["timezone"], "Europe/Madrid");
    assert!(times[1].get("utc_offset").is_some());
    assert!(times[1].get("unix_epoch").is_some());
}

#[test]
fn now_defaults_to_utc() {
    let output = Command::new(binary()).arg("now").output().expect("run now");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["times"][0]["timezone"], "UTC");
}

#[test]
fn invalid_timezone_returns_invalid_params() {
    let output = Command::new(binary())
        .args(["tz", "info", "Madrid"])
        .output()
        .expect("run invalid timezone");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stderr).expect("valid json");
    assert_eq!(json["error"]["error_code"], "INVALID_PARAMS");
    assert_eq!(json["error"]["details"]["value"], "Madrid");
}

#[test]
fn timezone_info_includes_dst_and_transition_metadata() {
    let output = Command::new(binary())
        .args(["tz", "info", "Europe/London"])
        .output()
        .expect("run timezone info");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["timezone"], "Europe/London");
    assert!(json["current"].get("is_dst").is_some());
    assert!(json["current"].get("utc_offset").is_some());
    assert!(json.get("next_transition").is_some());
}

#[test]
fn timezone_list_filters_region() {
    let output = Command::new(binary())
        .args(["tz", "list", "--region", "europe"])
        .output()
        .expect("run timezone list");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    let zones = json["timezones"].as_array().expect("timezone list");
    assert!(zones.iter().any(|zone| zone == "Europe/Madrid"));
    assert!(zones.iter().all(|zone| {
        zone.as_str()
            .expect("timezone string")
            .to_ascii_lowercase()
            .starts_with("europe/")
    }));
}

#[test]
fn convert_timezone_outputs_source_and_target() {
    let output = Command::new(binary())
        .args([
            "convert",
            "2026-06-18T12:00:00Z",
            "--from",
            "UTC",
            "--to",
            "Europe/Madrid",
        ])
        .output()
        .expect("run conversion");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["from_timezone"], "UTC");
    assert_eq!(json["to_timezone"], "Europe/Madrid");
    assert_eq!(
        json["target"]["local_datetime"],
        "2026-06-18T14:00:00+02:00"
    );
}

#[test]
fn calendar_outputs_expected_fields() {
    let output = Command::new(binary())
        .args(["calendar", "2026-06-18"])
        .output()
        .expect("run calendar");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["date"], "2026-06-18");
    assert_eq!(json["weekday"], "Thursday");
    assert_eq!(json["iso_week"], 25);
    assert_eq!(json["day_of_year"], 169);
    assert_eq!(json["days_in_month"], 30);
    assert_eq!(json["quarter"], 2);
}

#[test]
fn calc_diff_outputs_signed_machine_readable_units() {
    let output = Command::new(binary())
        .args(["calc", "diff", "2026-06-01", "2026-06-18"])
        .output()
        .expect("run calc diff");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["signed_days"], 17);
    assert_eq!(json["signed_weeks"], 2);
    assert_eq!(json["direction"], 1);
}

#[test]
fn format_outputs_rfc2822_for_absolute_datetime() {
    let output = Command::new(binary())
        .args([
            "format",
            "2026-06-18T12:00:00Z",
            "--output-format",
            "rfc2822",
        ])
        .output()
        .expect("run format");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["formatted"], "Thu, 18 Jun 2026 12:00:00 +0000");
    assert_eq!(json["timezone_present"], true);
}

#[test]
fn format_applies_utc_default_for_naive_datetime() {
    let output = Command::new(binary())
        .args([
            "format",
            "2026-06-18T12:00:00",
            "--output-format",
            "rfc2822",
        ])
        .output()
        .expect("run format");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["formatted"], "Thu, 18 Jun 2026 12:00:00 +0000");
    assert_eq!(json["timezone_present"], false);
}

#[test]
fn format_parses_fractional_naive_datetime_by_default() {
    let output = Command::new(binary())
        .args(["format", "2026-06-18T12:00:00.500"])
        .output()
        .expect("run format");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["formatted"], "2026-06-18T12:00:00.500Z");
    assert_eq!(json["timezone_present"], false);
}

#[test]
fn format_rejects_expanded_years_for_rfc3339() {
    let output = Command::new(binary())
        .args(["format", "+10000-01-01", "--output-format", "rfc3339"])
        .output()
        .expect("run format");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stderr).expect("valid json");
    assert_eq!(json["error"]["error_code"], "INVALID_PARAMS");
    assert_eq!(json["error"]["details"]["output_format"], "rfc3339");
}

#[test]
fn format_allows_expanded_years_for_iso8601() {
    let output = Command::new(binary())
        .args(["format", "+10000-01-01", "--output-format", "iso8601"])
        .output()
        .expect("run format");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["formatted"], "+10000-01-01T00:00:00Z");
}

#[test]
fn format_parses_expanded_offset_iso8601_by_default() {
    let output = Command::new(binary())
        .args([
            "format",
            "+10000-01-01T12:00:00Z",
            "--output-format",
            "iso8601",
        ])
        .output()
        .expect("run format");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["formatted"], "+10000-01-01T12:00:00Z");
    assert_eq!(json["timezone_present"], true);
}

#[test]
fn invalid_date_unit_returns_structured_error() {
    let output = Command::new(binary())
        .args(["calc", "add", "2026-06-18", "1", "fortnight"])
        .output()
        .expect("run invalid unit");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stderr).expect("valid json");
    assert_eq!(json["error"]["error_code"], "INVALID_PARAMS");
    assert_eq!(json["error"]["details"]["parameter"], "unit");
}

#[test]
fn invalid_calendar_date_returns_structured_error() {
    let output = Command::new(binary())
        .args(["calendar", "2026-02-30"])
        .output()
        .expect("run invalid calendar date");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stderr).expect("valid json");
    assert_eq!(json["error"]["error_code"], "INVALID_PARAMS");
}

#[test]
fn holiday_check_finds_us_christmas() {
    let output = Command::new(binary())
        .args(["holiday", "check", "2026-12-25", "--country", "US"])
        .output()
        .expect("run holiday check");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["date"], "2026-12-25");
    assert_eq!(json["country_code"], "US");
    assert_eq!(json["is_holiday"], true);
    assert_eq!(json["holiday"]["name"], "Christmas Day");
    assert_eq!(json["coverage"]["start_year"], 2000);
    assert_eq!(json["coverage"]["end_year"], 2030);
    assert_eq!(json["coverage"]["runtime_network"], false);
}

#[test]
fn holiday_list_outputs_gb_holidays() {
    let output = Command::new(binary())
        .args(["holiday", "list", "2026", "--country", "GB"])
        .output()
        .expect("run holiday list");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["country_code"], "GB");
    assert_eq!(json["year"], 2026);
    assert!(
        json["holidays"]
            .as_array()
            .expect("holiday list")
            .iter()
            .any(|holiday| holiday["date"] == "2026-12-25")
    );
}

#[test]
fn holiday_year_outside_coverage_returns_structured_error() {
    let output = Command::new(binary())
        .args(["holiday", "list", "2031", "--country", "US"])
        .output()
        .expect("run unsupported holiday year");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stderr).expect("valid json");
    assert_eq!(json["error"]["error_code"], "INVALID_PARAMS");
    assert_eq!(json["error"]["details"]["coverage_start_year"], 2000);
    assert_eq!(json["error"]["details"]["coverage_end_year"], 2030);
    assert_eq!(json["error"]["details"]["runtime_network"], false);
}

#[test]
fn unsupported_holiday_country_returns_structured_error() {
    let output = Command::new(binary())
        .args(["holiday", "check", "2026-12-25", "--country", "XX"])
        .output()
        .expect("run unsupported country");
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stderr).expect("valid json");
    assert_eq!(json["error"]["error_code"], "INVALID_PARAMS");
    assert_eq!(json["error"]["details"]["parameter"], "country");
    assert!(
        json["error"]["details"]["supported_countries"]
            .as_array()
            .expect("supported countries")
            .iter()
            .any(|country| country == "US")
    );
}

#[test]
fn business_days_between_defaults_to_weekend_only() {
    let output = Command::new(binary())
        .args([
            "biz",
            "between",
            "2026-12-24",
            "2026-12-28",
            "--country",
            "US",
        ])
        .output()
        .expect("run business days");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["business_days"], 3);
    assert_eq!(json["mode"], "weekend_only");
    assert_eq!(
        json["holidays_skipped"].as_array().expect("holidays").len(),
        0
    );
    assert!(json["coverage"].is_null());
}

#[test]
fn business_days_between_can_skip_country_holidays() {
    let output = Command::new(binary())
        .args([
            "biz",
            "between",
            "2026-12-24",
            "2026-12-28",
            "--country",
            "US",
            "--skip-holidays",
        ])
        .output()
        .expect("run holiday-aware business days");
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid json");
    assert_eq!(json["business_days"], 2);
    assert_eq!(json["mode"], "country_holidays");
    assert_eq!(json["holidays_skipped"][0]["date"], "2026-12-25");
}

#[test]
fn business_day_next_and_prev_are_strict() {
    let next = Command::new(binary())
        .args(["biz", "next", "2026-12-25", "--country", "US"])
        .output()
        .expect("run next business day");
    assert!(next.status.success());
    let json: serde_json::Value = serde_json::from_slice(&next.stdout).expect("valid json");
    assert_eq!(json["business_date"], "2026-12-28");
    assert_eq!(json["days_moved"], 3);

    let prev = Command::new(binary())
        .args(["biz", "prev", "2026-12-28", "--country", "US"])
        .output()
        .expect("run previous business day");
    assert!(prev.status.success());
    let json: serde_json::Value = serde_json::from_slice(&prev.stdout).expect("valid json");
    assert_eq!(json["business_date"], "2026-12-24");
    assert_eq!(json["days_moved"], 4);
}

#[test]
fn timer_crud_persists_across_processes() {
    let data_dir = temp_data_dir("timer-crud");
    let set = Command::new(binary())
        .args([
            "timer",
            "set",
            "q3-planning",
            "2026-07-01T17:00:00-04:00",
            "--description",
            "Q3 planning due",
            "--tag",
            "Work",
            "--tag",
            "planning",
        ])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .output()
        .expect("set timer");
    assert!(set.status.success());
    assert!(set.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&set.stdout).expect("valid json");
    assert_eq!(json["name"], "q3-planning");
    assert_eq!(json["deadline_utc"], "2026-07-01T21:00:00Z");
    assert_eq!(json["timezone"], "-04:00");
    assert_eq!(json["tags"][0], "planning");
    assert_eq!(json["tags"][1], "work");

    let get = Command::new(binary())
        .args(["timer", "get", "q3-planning"])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .output()
        .expect("get timer");
    assert!(get.status.success());
    let json: serde_json::Value = serde_json::from_slice(&get.stdout).expect("valid json");
    assert_eq!(json["description"], "Q3 planning due");
    assert_eq!(json["original_deadline"], "2026-07-01T17:00:00-04:00");

    let list = Command::new(binary())
        .args(["timer", "list", "--tag", "WORK"])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .output()
        .expect("list timers");
    assert!(list.status.success());
    let json: serde_json::Value = serde_json::from_slice(&list.stdout).expect("valid json");
    assert_eq!(json["tag"], "work");
    assert_eq!(json["count"], 1);
    assert_eq!(json["timers"][0]["name"], "q3-planning");

    let check = Command::new(binary())
        .args(["timer", "check"])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .output()
        .expect("check timers");
    assert!(check.status.success());
    let json: serde_json::Value = serde_json::from_slice(&check.stdout).expect("valid json");
    assert!(json["count"].as_u64().expect("count") <= 1);
}

#[test]
fn timer_naive_deadline_defaults_to_utc() {
    let data_dir = temp_data_dir("timer-naive-deadline");
    let set = Command::new(binary())
        .args(["timer", "set", "standup", "2026-07-01T17:00:00"])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .output()
        .expect("set timer");
    assert!(set.status.success());
    assert!(set.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&set.stdout).expect("valid json");
    assert_eq!(json["deadline_utc"], "2026-07-01T17:00:00Z");
    assert_eq!(json["original_deadline"], "2026-07-01T17:00:00");
    assert_eq!(json["timezone"], "+00:00");
}

#[test]
fn timer_delete_removes_timer_from_tag_filter() {
    let data_dir = temp_data_dir("timer-delete");
    let set = Command::new(binary())
        .args([
            "timer",
            "set",
            "delete-me",
            "2026-07-01T17:00:00Z",
            "--tag",
            "work",
        ])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .output()
        .expect("set timer");
    assert!(set.status.success());

    let delete = Command::new(binary())
        .args(["timer", "delete", "delete-me"])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .output()
        .expect("delete timer");
    assert!(delete.status.success());
    let json: serde_json::Value = serde_json::from_slice(&delete.stdout).expect("valid json");
    assert_eq!(json["deleted"], true);
    assert_eq!(json["deleted_tags"], 1);

    let list = Command::new(binary())
        .args(["timer", "list", "--tag", "work"])
        .env("TIME_KEEP_DATA_DIR", &data_dir)
        .output()
        .expect("list timers");
    assert!(list.status.success());
    let json: serde_json::Value = serde_json::from_slice(&list.stdout).expect("valid json");
    assert_eq!(json["count"], 0);
}

struct ChildGuard {
    child: Option<Child>,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn kill_and_wait(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        self.kill_and_wait();
    }
}

fn unused_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn poll_http_ok(port: u16, path: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(response) = try_http_request(
            port,
            &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n"),
        ) && response.starts_with("HTTP/1.1 200 OK")
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "HTTP server did not become ready"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn http_request(port: u16, request: &str) -> String {
    try_http_request(port, request).expect("HTTP request")
}

fn http_request_bytes(port: u16, request: &[u8]) -> String {
    try_http_request_bytes(port, request).expect("HTTP request")
}

fn mcp_http_post_json(port: u16, body: &str) -> serde_json::Value {
    let request = format!(
        "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let response = http_request(port, &request);
    assert!(
        response.starts_with("HTTP/1.1 200 OK"),
        "unexpected MCP response: {response:?}"
    );
    let (_, response_body) = response.split_once("\r\n\r\n").expect("HTTP body");
    serde_json::from_str(response_body).expect("JSON-RPC body")
}

fn mcp_text_payload(response: &serde_json::Value) -> serde_json::Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("text result");
    serde_json::from_str(text).expect("tool payload")
}

fn try_http_request(port: u16, request: &str) -> std::io::Result<String> {
    try_http_request_bytes(port, request.as_bytes())
}

fn try_http_request_bytes(port: u16, request: &[u8]) -> std::io::Result<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))?;
    stream.write_all(request)?;
    let mut response = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => response.extend_from_slice(&buffer[..count]),
            Err(err)
                if err.kind() == std::io::ErrorKind::ConnectionReset && !response.is_empty() =>
            {
                break;
            }
            Err(err) => return Err(err),
        }
    }
    String::from_utf8(response)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
}

fn temp_data_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "time-keep-cli-test-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp data dir");
    path
}
