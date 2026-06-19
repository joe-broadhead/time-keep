use std::{
    collections::BTreeMap,
    io::{self, BufRead, BufReader, Read, Write},
    net::{IpAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::{
    app::App,
    calendar,
    cli::{DateOutputFormat, ServerStartArgs, TimeFormat, Transport},
    db::TimerStore,
    error::{ErrorCode, Result, TimeKeepError},
    holidays,
    models::ErrorEnvelope,
    timezones,
    util::timer_db_path,
};

pub(crate) const MCP_PROTOCOL_VERSION: &str = "2025-03-26";
#[cfg(test)]
pub(crate) const TIMEZONE_TOOL_NAMES: [&str; 4] = [
    "current_time",
    "list_timezones",
    "timezone_info",
    "convert_timezone",
];
#[cfg(test)]
pub(crate) const DATE_TOOL_NAMES: [&str; 4] = [
    "calendar_query",
    "date_arithmetic",
    "date_diff",
    "date_format",
];
#[cfg(test)]
pub(crate) const HOLIDAY_TOOL_NAMES: [&str; 2] = ["holidays", "business_days"];
#[cfg(test)]
pub(crate) const TIMER_TOOL_NAMES: [&str; 5] = [
    "timer_set",
    "timer_get",
    "timer_list",
    "timer_delete",
    "timer_check",
];
pub(crate) const ALL_TOOL_NAMES: [&str; 15] = [
    "current_time",
    "list_timezones",
    "timezone_info",
    "timer_set",
    "timer_get",
    "timer_list",
    "timer_delete",
    "timer_check",
    "calendar_query",
    "holidays",
    "business_days",
    "date_arithmetic",
    "date_diff",
    "date_format",
    "convert_timezone",
];

const JSONRPC_VERSION: &str = "2.0";
const DEFAULT_MCP_PATH: &str = "/mcp";
const ACCEPT_SLEEP: Duration = Duration::from_millis(50);
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(5);
const HEADER_LIMIT_BYTES: usize = 16 * 1024;
const HEADER_LINE_LIMIT_BYTES: usize = 8 * 1024;
const BODY_LIMIT_BYTES: usize = 1_048_576;

pub(crate) fn run_server(app: &App, args: &ServerStartArgs) -> Result<()> {
    let runtime = Arc::new(McpRuntime::new(app.data_dir().to_path_buf()));
    match args.transport {
        Transport::Stdio => run_stdio(runtime),
        Transport::StreamableHttp => run_http(runtime, args),
    }
}

#[derive(Debug)]
struct McpRuntime {
    data_dir: PathBuf,
}

impl McpRuntime {
    fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    fn timer_store(&self) -> Result<TimerStore> {
        TimerStore::open(timer_db_path(&self.data_dir))
    }
}

fn run_stdio(runtime: Arc<McpRuntime>) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_jsonrpc_text(&runtime, &line) {
            serde_json::to_writer(&mut stdout, &response)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn run_http(runtime: Arc<McpRuntime>, args: &ServerStartArgs) -> Result<()> {
    let host = args.http_host.trim();
    if !is_loopback_host(host) {
        eprintln!(
            "warning: streamable HTTP is binding to non-loopback host {host}; use trusted networks and client authentication"
        );
    }

    let bind_address = bind_address(host, args.http_port);
    let listener = TcpListener::bind(&bind_address)?;
    listener.set_nonblocking(true)?;
    eprintln!(
        "time-keep MCP streamable HTTP listening on http://{}{}",
        display_address(host, args.http_port),
        normalize_http_path(&args.http_path)
    );

    let shutdown = Arc::new(AtomicBool::new(false));
    install_shutdown_handler(Arc::clone(&shutdown));
    let active = Arc::new(AtomicUsize::new(0));
    let mcp_path = normalize_http_path(&args.http_path);

    while !shutdown.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let runtime = Arc::clone(&runtime);
                let active = Arc::clone(&active);
                let mcp_path = mcp_path.clone();
                let host = host.to_string();
                active.fetch_add(1, Ordering::SeqCst);
                thread::spawn(move || {
                    if let Err(err) = handle_http_connection(stream, &runtime, &mcp_path, &host) {
                        eprintln!("warning: MCP HTTP request failed: {err}");
                    }
                    active.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_SLEEP);
            }
            Err(err) => return Err(err.into()),
        }
    }

    while active.load(Ordering::SeqCst) > 0 {
        thread::sleep(ACCEPT_SLEEP);
    }

    Ok(())
}

fn install_shutdown_handler(shutdown: Arc<AtomicBool>) {
    if let Err(err) = ctrlc::set_handler(move || {
        shutdown.store(true, Ordering::SeqCst);
    }) {
        eprintln!("warning: failed to install shutdown handler: {err}");
    }
}

fn handle_http_connection(
    mut stream: TcpStream,
    runtime: &Arc<McpRuntime>,
    mcp_path: &str,
    host: &str,
) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(HEADER_READ_TIMEOUT))?;
    let request = match read_http_request(&mut stream) {
        Ok(Some(request)) => request,
        Ok(None) => return Ok(()),
        Err(err) if err.kind() == io::ErrorKind::InvalidData => {
            let body = format!("bad request: {err}\n");
            return write_http_response(
                &mut stream,
                400,
                "Bad Request",
                "text/plain; charset=utf-8",
                body.as_bytes(),
            );
        }
        Err(err) => return Err(err),
    };

    if !origin_is_allowed(&request.headers, host) {
        return write_http_response(
            &mut stream,
            403,
            "Forbidden",
            "text/plain; charset=utf-8",
            b"forbidden\n",
        );
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/healthz") => {
            write_http_response(&mut stream, 200, "OK", "text/plain; charset=utf-8", b"ok\n")
        }
        ("GET", "/readyz") => write_http_response(
            &mut stream,
            200,
            "OK",
            "text/plain; charset=utf-8",
            b"ready\n",
        ),
        ("POST", path) if path == mcp_path => {
            let text = match String::from_utf8(request.body) {
                Ok(text) => text,
                Err(err) => {
                    let body = format!("bad request: invalid UTF-8 body: {err}\n");
                    return write_http_response(
                        &mut stream,
                        400,
                        "Bad Request",
                        "text/plain; charset=utf-8",
                        body.as_bytes(),
                    );
                }
            };
            match handle_jsonrpc_text(runtime, &text) {
                Some(response) => {
                    let body = serde_json::to_vec(&response).map_err(io::Error::other)?;
                    write_http_response(
                        &mut stream,
                        200,
                        "OK",
                        "application/json; charset=utf-8",
                        &body,
                    )
                }
                None => write_http_response(&mut stream, 202, "Accepted", "text/plain", b""),
            }
        }
        ("GET", path) if path == mcp_path => write_http_response(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"streaming GET is not implemented; use JSON-RPC POST\n",
        ),
        _ => write_http_response(
            &mut stream,
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            b"not found\n",
        ),
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut TcpStream) -> io::Result<Option<HttpRequest>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut header_bytes = 0;
    let Some(request_line) = read_limited_http_line(&mut reader, &mut header_bytes)? else {
        return Ok(None);
    };
    let request_line = request_line.trim_end_matches(['\r', '\n']);
    if request_line.is_empty() {
        return Ok(None);
    }

    let mut parts = request_line.split_whitespace();
    let Some(method) = parts.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing HTTP method",
        ));
    };
    let Some(target) = parts.next() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing HTTP target",
        ));
    };
    let path = target.split_once('?').map_or(target, |(path, _query)| path);

    let mut headers = BTreeMap::new();
    loop {
        let Some(line) = read_limited_http_line(&mut reader, &mut header_bytes)? else {
            break;
        };
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }

    let content_length = headers
        .get("content-length")
        .map(|value| {
            value.parse::<usize>().map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid content-length: {err}"),
                )
            })
        })
        .transpose()?
        .unwrap_or(0);
    if content_length > BODY_LIMIT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("request body exceeds {BODY_LIMIT_BYTES} bytes"),
        ));
    }

    let mut body = vec![0; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    Ok(Some(HttpRequest {
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
        headers,
        body,
    }))
}

fn read_limited_http_line(
    reader: &mut BufReader<TcpStream>,
    header_bytes: &mut usize,
) -> io::Result<Option<String>> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            if line.is_empty() {
                return Ok(None);
            }
            break;
        }

        let next_newline = buffer
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|position| position + 1)
            .unwrap_or(buffer.len());
        let remaining_line = HEADER_LINE_LIMIT_BYTES.saturating_sub(line.len());
        let remaining_headers = HEADER_LIMIT_BYTES.saturating_sub(*header_bytes);
        let to_take = next_newline.min(remaining_line).min(remaining_headers);
        if to_take == 0 {
            return Err(header_limit_error());
        }

        line.extend_from_slice(&buffer[..to_take]);
        reader.consume(to_take);
        *header_bytes += to_take;

        if to_take < next_newline {
            return Err(header_limit_error());
        }
        if line.ends_with(b"\n") {
            break;
        }
    }

    String::from_utf8(line)
        .map(Some)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

fn header_limit_error() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "HTTP headers exceed size limit")
}

fn write_http_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    let mut response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    stream.write_all(&response)?;
    stream.flush()
}

fn handle_jsonrpc_text(runtime: &Arc<McpRuntime>, text: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(text) {
        Ok(value) => handle_jsonrpc_value(runtime, value),
        Err(_) => Some(jsonrpc_error(Value::Null, -32700, "Parse error")),
    }
}

fn handle_jsonrpc_value(runtime: &Arc<McpRuntime>, value: Value) -> Option<Value> {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                return Some(jsonrpc_error(Value::Null, -32600, "Invalid Request"));
            }

            let responses = items
                .into_iter()
                .filter_map(|item| handle_jsonrpc_single(runtime, item))
                .collect::<Vec<_>>();
            if responses.is_empty() {
                None
            } else {
                Some(Value::Array(responses))
            }
        }
        other => handle_jsonrpc_single(runtime, other),
    }
}

fn handle_jsonrpc_single(runtime: &Arc<McpRuntime>, value: Value) -> Option<Value> {
    let Value::Object(object) = value else {
        return Some(jsonrpc_error(Value::Null, -32600, "Invalid Request"));
    };

    let Some(method_value) = object.get("method") else {
        if object.contains_key("result") || object.contains_key("error") {
            return None;
        }
        let id = jsonrpc_id(&object).unwrap_or(Value::Null);
        return Some(jsonrpc_error(id, -32600, "Invalid Request"));
    };
    let id = jsonrpc_id(&object);
    if object.get("jsonrpc").and_then(Value::as_str) != Some(JSONRPC_VERSION) {
        return Some(jsonrpc_error(
            id.clone().unwrap_or(Value::Null),
            -32600,
            "Invalid Request",
        ));
    }
    let method = match method_value.as_str() {
        Some(method) => method,
        None => {
            return id.map(|id| jsonrpc_error(id, -32600, "Invalid Request"));
        }
    };

    let response = match method {
        "initialize" => Ok(initialize_result()),
        "notifications/initialized" => Ok(Value::Null),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => tools_call_result(runtime, object.get("params")),
        _ => Err(JsonRpcError::new(-32601, "Method not found")),
    };

    let id = id?;
    Some(match response {
        Ok(Value::Null) if method == "notifications/initialized" => return None,
        Ok(result) => json!({
            "jsonrpc": JSONRPC_VERSION,
            "id": id,
            "result": result,
        }),
        Err(err) => jsonrpc_error(id, err.code, err.message),
    })
}

fn jsonrpc_id(object: &Map<String, Value>) -> Option<Value> {
    object.get("id").cloned()
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": crate::APP_NAME,
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Local-first time, timezone, calendar, business-day, holiday, and timer tools."
    })
}

fn tools_call_result(
    runtime: &Arc<McpRuntime>,
    params: Option<&Value>,
) -> std::result::Result<Value, JsonRpcError> {
    let Some(Value::Object(params)) = params else {
        return Err(JsonRpcError::new(-32602, "Invalid params"));
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err(JsonRpcError::new(-32602, "Invalid params"));
    };
    let arguments = match params.get("arguments") {
        Some(Value::Object(arguments)) => arguments,
        Some(Value::Null) | None => {
            return Ok(tool_result(call_tool(runtime, name, &Map::new())));
        }
        Some(_) => return Err(JsonRpcError::new(-32602, "Invalid params")),
    };

    Ok(tool_result(call_tool(runtime, name, arguments)))
}

fn validate_tool_arguments(name: &str, args: &Map<String, Value>) -> Result<()> {
    let Some(allowed) = allowed_tool_arguments(name) else {
        return Ok(());
    };
    for key in args.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(unknown_arg_error(name, key, allowed));
        }
    }
    Ok(())
}

fn allowed_tool_arguments(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "current_time" => Some(&["timezones", "format"]),
        "list_timezones" => Some(&["region"]),
        "timezone_info" => Some(&["timezone"]),
        "convert_timezone" => Some(&["datetime", "from_timezone", "to_timezone"]),
        "calendar_query" => Some(&["date"]),
        "date_arithmetic" => Some(&["operation", "date", "amount", "unit"]),
        "date_diff" => Some(&["from", "to"]),
        "date_format" => Some(&["input", "output_format", "strftime", "input_format"]),
        "holidays" => Some(&["action", "country", "date", "year"]),
        "business_days" => Some(&["action", "from", "to", "date", "country", "skip_holidays"]),
        "timer_set" => Some(&["name", "deadline", "description", "tags"]),
        "timer_get" | "timer_delete" => Some(&["name"]),
        "timer_list" => Some(&["tag"]),
        "timer_check" => Some(&[]),
        _ => None,
    }
}

fn call_tool(runtime: &McpRuntime, name: &str, args: &Map<String, Value>) -> Result<Value> {
    validate_tool_arguments(name, args)?;

    match name {
        "current_time" => {
            let timezones = optional_string_array(args, "timezones")?.unwrap_or_default();
            let format = parse_time_format(optional_string(args, "format")?.as_deref())?;
            serde_json::to_value(timezones::current_time(&timezones, format)?).map_err(Into::into)
        }
        "list_timezones" => {
            let region = optional_string(args, "region")?;
            serde_json::to_value(timezones::list_timezones(region.as_deref())).map_err(Into::into)
        }
        "timezone_info" => {
            let timezone = required_string(args, "timezone")?;
            serde_json::to_value(timezones::timezone_info(&timezone)?).map_err(Into::into)
        }
        "convert_timezone" => {
            let datetime = required_string(args, "datetime")?;
            let from_timezone = required_string(args, "from_timezone")?;
            let to_timezone = required_string(args, "to_timezone")?;
            serde_json::to_value(timezones::convert_timezone(
                &datetime,
                &from_timezone,
                &to_timezone,
            )?)
            .map_err(Into::into)
        }
        "calendar_query" => {
            let date = required_string(args, "date")?;
            serde_json::to_value(calendar::calendar_query(&date)?).map_err(Into::into)
        }
        "date_arithmetic" => {
            let operation = required_string(args, "operation")?;
            let date = required_string(args, "date")?;
            let amount = required_i64(args, "amount")?;
            let unit = required_string(args, "unit")?;
            let result = match operation.as_str() {
                "add" => calendar::add(&date, amount, &unit),
                "subtract" => calendar::subtract(&date, amount, &unit),
                _ => Err(invalid_value_error(
                    "operation",
                    &operation,
                    &["add", "subtract"],
                )),
            }?;
            serde_json::to_value(result).map_err(Into::into)
        }
        "date_diff" => {
            let from = required_string(args, "from")?;
            let to = required_string(args, "to")?;
            serde_json::to_value(calendar::diff(&from, &to)?).map_err(Into::into)
        }
        "date_format" => {
            let input = required_string(args, "input")?;
            let output_format =
                parse_date_output_format(optional_string(args, "output_format")?.as_deref())?;
            let strftime = optional_string(args, "strftime")?;
            let input_format = optional_string(args, "input_format")?;
            serde_json::to_value(calendar::format_datetime(
                &input,
                output_format,
                strftime.as_deref(),
                input_format.as_deref(),
            )?)
            .map_err(Into::into)
        }
        "holidays" => {
            let action = required_string(args, "action")?;
            let country = required_string(args, "country")?;
            match action.as_str() {
                "check" => {
                    let date = required_string(args, "date")?;
                    serde_json::to_value(holidays::holiday_check(&date, &country)?)
                        .map_err(Into::into)
                }
                "list" => {
                    let year = required_i32(args, "year")?;
                    serde_json::to_value(holidays::holiday_list(year, &country)?)
                        .map_err(Into::into)
                }
                _ => Err(invalid_value_error("action", &action, &["check", "list"])),
            }
        }
        "business_days" => {
            let action = required_string(args, "action")?;
            match action.as_str() {
                "between" => {
                    let from = required_string(args, "from")?;
                    let to = required_string(args, "to")?;
                    let country = optional_string(args, "country")?;
                    let skip_holidays = optional_bool(args, "skip_holidays")?.unwrap_or(false);
                    serde_json::to_value(holidays::business_days_between(
                        &from,
                        &to,
                        country.as_deref(),
                        skip_holidays,
                    )?)
                    .map_err(Into::into)
                }
                "next" => {
                    let date = required_string(args, "date")?;
                    let country = optional_string(args, "country")?;
                    serde_json::to_value(holidays::next_business_day(&date, country.as_deref())?)
                        .map_err(Into::into)
                }
                "prev" | "previous" => {
                    let date = required_string(args, "date")?;
                    let country = optional_string(args, "country")?;
                    serde_json::to_value(holidays::previous_business_day(
                        &date,
                        country.as_deref(),
                    )?)
                    .map_err(Into::into)
                }
                _ => Err(invalid_value_error(
                    "action",
                    &action,
                    &["between", "next", "prev"],
                )),
            }
        }
        "timer_set" => {
            let name = required_string(args, "name")?;
            let deadline = required_string(args, "deadline")?;
            let description = optional_string(args, "description")?;
            let tags = optional_string_array(args, "tags")?.unwrap_or_default();
            let mut store = runtime.timer_store()?;
            serde_json::to_value(store.set_timer(
                &name,
                &deadline,
                description.as_deref(),
                &tags,
            )?)
            .map_err(Into::into)
        }
        "timer_get" => {
            let name = required_string(args, "name")?;
            let store = runtime.timer_store()?;
            serde_json::to_value(store.get_timer(&name)?).map_err(Into::into)
        }
        "timer_list" => {
            let tag = optional_string(args, "tag")?;
            let store = runtime.timer_store()?;
            serde_json::to_value(store.list_timers(tag.as_deref())?).map_err(Into::into)
        }
        "timer_delete" => {
            let name = required_string(args, "name")?;
            let mut store = runtime.timer_store()?;
            serde_json::to_value(store.delete_timer(&name)?).map_err(Into::into)
        }
        "timer_check" => {
            let store = runtime.timer_store()?;
            serde_json::to_value(store.check_timers()?).map_err(Into::into)
        }
        _ => Err(
            TimeKeepError::invalid_params(format!("unknown MCP tool: {name}"))
                .with_detail("parameter", json!("name"))
                .with_detail("value", json!(name))
                .with_detail("tools", json!(ALL_TOOL_NAMES)),
        ),
    }
}

fn tool_result(result: Result<Value>) -> Value {
    match result {
        Ok(value) => json!({
            "content": [{
                "type": "text",
                "text": json_text(&value)
            }],
            "isError": false
        }),
        Err(err) => {
            let envelope = ErrorEnvelope::from(&err);
            json!({
                "content": [{
                    "type": "text",
                    "text": json_text(&envelope)
                }],
                "isError": true
            })
        }
    }
}

fn json_text(value: &impl Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|err| {
        json!({
            "error": {
                "error_code": ErrorCode::Internal.as_str(),
                "message": format!("failed to serialize MCP tool result: {err}")
            }
        })
        .to_string()
    })
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "current_time",
            "description": "Show current time in one or more IANA timezones.",
            "inputSchema": object_schema(
                vec![
                    ("timezones", array_schema("IANA timezone names. Defaults to UTC.")),
                    ("format", enum_schema(&["rfc3339", "iso8601", "epoch"])),
                ],
                &[],
            )
        }),
        json!({
            "name": "list_timezones",
            "description": "List IANA timezone names, optionally filtered by region.",
            "inputSchema": object_schema(vec![("region", string_schema("Optional region such as europe or america."))], &[])
        }),
        json!({
            "name": "timezone_info",
            "description": "Show current timezone metadata and next transition.",
            "inputSchema": object_schema(vec![("timezone", string_schema("IANA timezone name."))], &["timezone"])
        }),
        json!({
            "name": "timer_set",
            "description": "Create or update a local SQLite timer.",
            "inputSchema": object_schema(
                vec![
                    ("name", string_schema("Timer name.")),
                    ("deadline", string_schema("ISO 8601/RFC3339 deadline.")),
                    ("description", string_schema("Optional timer description.")),
                    ("tags", array_schema("Timer tags.")),
                ],
                &["name", "deadline"],
            )
        }),
        json!({
            "name": "timer_get",
            "description": "Read one local timer.",
            "inputSchema": object_schema(vec![("name", string_schema("Timer name."))], &["name"])
        }),
        json!({
            "name": "timer_list",
            "description": "List local timers with optional tag filtering.",
            "inputSchema": object_schema(vec![("tag", string_schema("Normalized tag filter."))], &[])
        }),
        json!({
            "name": "timer_delete",
            "description": "Delete one local timer.",
            "inputSchema": object_schema(vec![("name", string_schema("Timer name."))], &["name"])
        }),
        json!({
            "name": "timer_check",
            "description": "List overdue local timers.",
            "inputSchema": object_schema(Vec::new(), &[])
        }),
        json!({
            "name": "calendar_query",
            "description": "Query calendar fields for an ISO date.",
            "inputSchema": object_schema(vec![("date", string_schema("ISO date such as 2026-06-18."))], &["date"])
        }),
        json!({
            "name": "holidays",
            "description": "Check or list bounded offline holidays.",
            "inputSchema": object_schema(
                vec![
                    ("action", enum_schema(&["check", "list"])),
                    ("country", string_schema("ISO 3166-1 alpha-2 country code.")),
                    ("date", string_schema("ISO date for check.")),
                    ("year", integer_schema("Year for list.")),
                ],
                &["action", "country"],
            )
        }),
        json!({
            "name": "business_days",
            "description": "Count or search business days with optional offline holiday skipping.",
            "inputSchema": object_schema(
                vec![
                    ("action", enum_schema(&["between", "next", "prev"])),
                    ("from", string_schema("Start ISO date for between.")),
                    ("to", string_schema("End ISO date for between.")),
                    ("date", string_schema("ISO date for next or prev.")),
                    ("country", string_schema("Optional ISO 3166-1 alpha-2 country code.")),
                    ("skip_holidays", bool_schema("Skip holidays for between.")),
                ],
                &["action"],
            )
        }),
        json!({
            "name": "date_arithmetic",
            "description": "Add or subtract a duration from a date or datetime.",
            "inputSchema": object_schema(
                vec![
                    ("operation", enum_schema(&["add", "subtract"])),
                    ("date", string_schema("Date or datetime.")),
                    ("amount", integer_schema("Amount to add or subtract.")),
                    ("unit", enum_schema(&["seconds", "minutes", "hours", "days", "weeks", "months", "years"])),
                ],
                &["operation", "date", "amount", "unit"],
            )
        }),
        json!({
            "name": "date_diff",
            "description": "Calculate the difference between two dates or datetimes.",
            "inputSchema": object_schema(
                vec![
                    ("from", string_schema("Start date or datetime.")),
                    ("to", string_schema("End date or datetime.")),
                ],
                &["from", "to"],
            )
        }),
        json!({
            "name": "date_format",
            "description": "Parse and format a date or datetime.",
            "inputSchema": object_schema(
                vec![
                    ("input", string_schema("Datetime input.")),
                    ("output_format", enum_schema(&["iso8601", "rfc3339", "rfc2822", "epoch", "unix_timestamp", "strftime"])),
                    ("strftime", string_schema("strftime pattern when output_format is strftime.")),
                    ("input_format", string_schema("Optional input format hint.")),
                ],
                &["input"],
            )
        }),
        json!({
            "name": "convert_timezone",
            "description": "Convert a datetime between IANA timezones.",
            "inputSchema": object_schema(
                vec![
                    ("datetime", string_schema("Datetime to convert.")),
                    ("from_timezone", string_schema("Source IANA timezone.")),
                    ("to_timezone", string_schema("Target IANA timezone.")),
                ],
                &["datetime", "from_timezone", "to_timezone"],
            )
        }),
    ]
}

fn object_schema(properties: Vec<(&str, Value)>, required: &[&str]) -> Value {
    let properties = properties
        .into_iter()
        .map(|(name, schema)| (name.to_string(), schema))
        .collect::<Map<_, _>>();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn string_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description,
    })
}

fn array_schema(description: &str) -> Value {
    json!({
        "type": "array",
        "description": description,
        "items": {
            "type": "string"
        }
    })
}

fn integer_schema(description: &str) -> Value {
    json!({
        "type": "integer",
        "description": description,
    })
}

fn bool_schema(description: &str) -> Value {
    json!({
        "type": "boolean",
        "description": description,
    })
}

fn enum_schema(values: &[&str]) -> Value {
    json!({
        "type": "string",
        "enum": values,
    })
}

fn required_string(args: &Map<String, Value>, key: &'static str) -> Result<String> {
    optional_string(args, key)?.ok_or_else(|| missing_arg_error(key))
}

fn optional_string(args: &Map<String, Value>, key: &'static str) -> Result<Option<String>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(value) => Err(type_arg_error(key, "string", value)),
    }
}

fn optional_string_array(
    args: &Map<String, Value>,
    key: &'static str,
) -> Result<Option<Vec<String>>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| type_arg_error(key, "array of strings", value))
            })
            .collect::<Result<Vec<_>>>()
            .map(Some),
        Some(value) => Err(type_arg_error(key, "array of strings", value)),
    }
}

fn optional_bool(args: &Map<String, Value>, key: &'static str) -> Result<Option<bool>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(value) => Err(type_arg_error(key, "boolean", value)),
    }
}

fn required_i64(args: &Map<String, Value>, key: &'static str) -> Result<i64> {
    match args.get(key) {
        Some(Value::Number(number)) => number
            .as_i64()
            .ok_or_else(|| type_arg_error(key, "integer", args.get(key).unwrap_or(&Value::Null))),
        Some(value) => Err(type_arg_error(key, "integer", value)),
        None => Err(missing_arg_error(key)),
    }
}

fn required_i32(args: &Map<String, Value>, key: &'static str) -> Result<i32> {
    let value = required_i64(args, key)?;
    i32::try_from(value).map_err(|_| {
        TimeKeepError::invalid_params(format!("{key} is outside supported i32 range"))
            .with_detail("parameter", json!(key))
            .with_detail("value", json!(value))
    })
}

fn parse_time_format(input: Option<&str>) -> Result<TimeFormat> {
    match input.map(normalize_enum_value).as_deref() {
        None | Some("rfc3339") => Ok(TimeFormat::Rfc3339),
        Some("iso8601") | Some("iso_8601") => Ok(TimeFormat::Iso8601),
        Some("epoch") => Ok(TimeFormat::Epoch),
        Some(value) => Err(invalid_value_error(
            "format",
            value,
            &["rfc3339", "iso8601", "epoch"],
        )),
    }
}

fn parse_date_output_format(input: Option<&str>) -> Result<DateOutputFormat> {
    match input.map(normalize_enum_value).as_deref() {
        None | Some("rfc3339") => Ok(DateOutputFormat::Rfc3339),
        Some("iso8601") | Some("iso_8601") => Ok(DateOutputFormat::Iso8601),
        Some("rfc2822") => Ok(DateOutputFormat::Rfc2822),
        Some("epoch") => Ok(DateOutputFormat::Epoch),
        Some("unix_timestamp") => Ok(DateOutputFormat::UnixTimestamp),
        Some("strftime") => Ok(DateOutputFormat::Strftime),
        Some(value) => Err(invalid_value_error(
            "output_format",
            value,
            &[
                "iso8601",
                "rfc3339",
                "rfc2822",
                "epoch",
                "unix_timestamp",
                "strftime",
            ],
        )),
    }
}

fn normalize_enum_value(input: &str) -> String {
    input.trim().to_ascii_lowercase().replace('-', "_")
}

fn missing_arg_error(parameter: &'static str) -> TimeKeepError {
    TimeKeepError::invalid_params(format!("missing required MCP tool parameter: {parameter}"))
        .with_detail("parameter", json!(parameter))
}

fn type_arg_error(parameter: &'static str, expected: &'static str, value: &Value) -> TimeKeepError {
    TimeKeepError::invalid_params(format!(
        "invalid MCP tool parameter type for {parameter}; expected {expected}"
    ))
    .with_detail("parameter", json!(parameter))
    .with_detail("expected", json!(expected))
    .with_detail("value", value.clone())
}

fn invalid_value_error(parameter: &'static str, value: &str, allowed: &[&str]) -> TimeKeepError {
    TimeKeepError::invalid_params(format!(
        "invalid MCP tool parameter value for {parameter}: {value}"
    ))
    .with_detail("parameter", json!(parameter))
    .with_detail("value", json!(value))
    .with_detail("allowed", json!(allowed))
}

fn unknown_arg_error(tool: &str, parameter: &str, allowed: &[&str]) -> TimeKeepError {
    TimeKeepError::invalid_params(format!(
        "unknown MCP tool parameter for {tool}: {parameter}"
    ))
    .with_detail("tool", json!(tool))
    .with_detail("parameter", json!(parameter))
    .with_detail("allowed", json!(allowed))
}

#[derive(Debug)]
struct JsonRpcError {
    code: i64,
    message: &'static str,
}

impl JsonRpcError {
    fn new(code: i64, message: &'static str) -> Self {
        Self { code, message }
    }
}

fn jsonrpc_error(id: Value, code: i64, message: &'static str) -> Value {
    json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn normalize_http_path(path: &str) -> String {
    let path = path.trim();
    if path.is_empty() {
        DEFAULT_MCP_PATH.to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

fn bind_address(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn display_address(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn is_loopback_host(host: &str) -> bool {
    let host = normalize_host(host);
    host == "localhost"
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn origin_is_allowed(headers: &BTreeMap<String, String>, configured_host: &str) -> bool {
    let Some(origin) = headers.get("origin") else {
        return true;
    };
    let Some(origin_host) = host_from_origin(origin) else {
        return false;
    };
    is_loopback_host(&origin_host)
        || normalize_host(&origin_host) == normalize_host(configured_host)
}

fn host_from_origin(origin: &str) -> Option<String> {
    let without_scheme = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))?;
    let authority = without_scheme.split('/').next().unwrap_or(without_scheme);
    if authority.starts_with('[') {
        let end = authority.find(']')?;
        Some(authority[1..end].to_string())
    } else {
        Some(authority.split(':').next().unwrap_or(authority).to_string())
    }
}

fn normalize_host(host: &str) -> String {
    host.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_name_groups_cover_all_planned_tools() {
        let mut grouped = Vec::new();
        grouped.extend(TIMEZONE_TOOL_NAMES);
        grouped.extend(DATE_TOOL_NAMES);
        grouped.extend(HOLIDAY_TOOL_NAMES);
        grouped.extend(TIMER_TOOL_NAMES);
        grouped.sort_unstable();

        let mut all = ALL_TOOL_NAMES.to_vec();
        all.sort_unstable();

        assert_eq!(grouped, all);
        assert_eq!(ALL_TOOL_NAMES.len(), 15);
    }

    #[test]
    fn mcp_protocol_lists_exact_tool_names() {
        let runtime = Arc::new(McpRuntime::new(temp_data_dir("list")));
        let response = handle_jsonrpc_text(
            &runtime,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        )
        .expect("response");
        let tools = response["result"]["tools"].as_array().expect("tools array");
        let names = tools
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<Vec<_>>();
        assert_eq!(names, ALL_TOOL_NAMES);
    }

    #[test]
    fn mcp_protocol_rejects_missing_jsonrpc_version() {
        let runtime = Arc::new(McpRuntime::new(temp_data_dir("missing-jsonrpc")));
        let response =
            handle_jsonrpc_text(&runtime, r#"{"id":1,"method":"tools/list"}"#).expect("response");
        assert_eq!(response["id"], 1);
        assert_eq!(response["error"]["code"], -32600);
        assert_eq!(response["error"]["message"], "Invalid Request");
        assert!(response.get("result").is_none());
    }

    #[test]
    fn mcp_protocol_batches_responses_and_skips_notifications() {
        let runtime = Arc::new(McpRuntime::new(temp_data_dir("batch")));
        let response = handle_jsonrpc_text(
            &runtime,
            r#"[
                {"jsonrpc":"2.0","method":"notifications/initialized","params":{}},
                {"jsonrpc":"2.0","id":"ping","method":"ping"},
                {"jsonrpc":"2.0","id":"missing","method":"not-a-method"}
            ]"#,
        )
        .expect("batch response");

        let responses = response.as_array().expect("batch array");
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0]["id"], "ping");
        assert_eq!(responses[0]["result"], json!({}));
        assert_eq!(responses[1]["id"], "missing");
        assert_eq!(responses[1]["error"]["code"], -32601);
    }

    #[test]
    fn mcp_protocol_calls_current_time() {
        let runtime = Arc::new(McpRuntime::new(temp_data_dir("current-time")));
        let response = handle_jsonrpc_text(
            &runtime,
            r#"{"jsonrpc":"2.0","id":"call","method":"tools/call","params":{"name":"current_time","arguments":{"timezones":["UTC"],"format":"rfc3339"}}}"#,
        )
        .expect("response");
        assert_eq!(response["result"]["isError"], false);
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text result");
        let payload: Value = serde_json::from_str(text).expect("tool payload");
        assert_eq!(payload["times"][0]["timezone"], "UTC");
        assert!(payload["generated_at_utc"].is_string());
    }

    #[test]
    fn mcp_protocol_returns_structured_tool_error() {
        let runtime = Arc::new(McpRuntime::new(temp_data_dir("tool-error")));
        let response = handle_jsonrpc_text(
            &runtime,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"timezone_info","arguments":{"timezone":"Madrid"}}}"#,
        )
        .expect("response");
        assert_eq!(response["result"]["isError"], true);
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text result");
        let payload: Value = serde_json::from_str(text).expect("error payload");
        assert_eq!(payload["error"]["error_code"], "INVALID_PARAMS");
        assert_eq!(payload["error"]["details"]["value"], "Madrid");
    }

    #[test]
    fn mcp_protocol_rejects_unknown_tool_argument() {
        let runtime = Arc::new(McpRuntime::new(temp_data_dir("unknown-argument")));
        let response = handle_jsonrpc_text(
            &runtime,
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"current_time","arguments":{"timezone":"UTC"}}}"#,
        )
        .expect("response");
        assert_eq!(response["result"]["isError"], true);
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text result");
        let payload: Value = serde_json::from_str(text).expect("error payload");
        assert_eq!(payload["error"]["error_code"], "INVALID_PARAMS");
        assert_eq!(payload["error"]["details"]["tool"], "current_time");
        assert_eq!(payload["error"]["details"]["parameter"], "timezone");
        assert_eq!(payload["error"]["details"]["allowed"][0], "timezones");
    }

    #[test]
    fn mcp_protocol_tool_schemas_match_argument_allowlists() {
        let tools = tool_definitions();
        let names = tools
            .iter()
            .map(|tool| tool["name"].as_str().expect("tool name"))
            .collect::<Vec<_>>();
        assert_eq!(names, ALL_TOOL_NAMES);

        for tool in tools {
            let name = tool["name"].as_str().expect("tool name");
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object", "{name} schema type");
            assert_eq!(
                schema["additionalProperties"], false,
                "{name} must reject unknown arguments"
            );

            let properties = schema["properties"].as_object().expect("properties");
            let mut property_names = properties.keys().map(String::as_str).collect::<Vec<_>>();
            property_names.sort_unstable();

            let mut allowed = allowed_tool_arguments(name).expect("known tool").to_vec();
            allowed.sort_unstable();
            assert_eq!(property_names, allowed, "{name} argument schema drifted");

            for required in schema["required"].as_array().expect("required array") {
                let required = required.as_str().expect("required name");
                assert!(
                    properties.contains_key(required),
                    "{name} required argument {required} is missing from properties"
                );
            }
        }
    }

    #[test]
    fn mcp_protocol_timer_tools_persist_in_runtime_data_dir() {
        let runtime = Arc::new(McpRuntime::new(temp_data_dir("timer-persistence")));
        let set = handle_jsonrpc_text(
            &runtime,
            r#"{"jsonrpc":"2.0","id":"set","method":"tools/call","params":{"name":"timer_set","arguments":{"name":"release-check","deadline":"2026-07-01T17:00:00-04:00","description":"Release smoke","tags":["Work","release"]}}}"#,
        )
        .expect("set response");
        assert_eq!(set["result"]["isError"], false);
        let payload = tool_payload(&set);
        assert_eq!(payload["name"], "release-check");
        assert_eq!(payload["deadline_utc"], "2026-07-01T21:00:00Z");
        assert_eq!(payload["tags"][0], "release");
        assert_eq!(payload["tags"][1], "work");

        let list = handle_jsonrpc_text(
            &runtime,
            r#"{"jsonrpc":"2.0","id":"list","method":"tools/call","params":{"name":"timer_list","arguments":{"tag":"WORK"}}}"#,
        )
        .expect("list response");
        let payload = tool_payload(&list);
        assert_eq!(payload["tag"], "work");
        assert_eq!(payload["count"], 1);
        assert_eq!(payload["timers"][0]["name"], "release-check");

        let get = handle_jsonrpc_text(
            &runtime,
            r#"{"jsonrpc":"2.0","id":"get","method":"tools/call","params":{"name":"timer_get","arguments":{"name":"release-check"}}}"#,
        )
        .expect("get response");
        let payload = tool_payload(&get);
        assert_eq!(payload["description"], "Release smoke");
        assert_eq!(payload["original_deadline"], "2026-07-01T17:00:00-04:00");
    }

    #[test]
    fn origin_validation_accepts_loopback_and_rejects_remote() {
        let mut headers = BTreeMap::new();
        headers.insert("origin".to_string(), "http://localhost:3000".to_string());
        assert!(origin_is_allowed(&headers, "127.0.0.1"));

        headers.insert("origin".to_string(), "https://example.com".to_string());
        assert!(!origin_is_allowed(&headers, "127.0.0.1"));

        headers.insert(
            "origin".to_string(),
            "http://127.attacker.example".to_string(),
        );
        assert!(!origin_is_allowed(&headers, "127.0.0.1"));
        assert!(!is_loopback_host("127.attacker.example"));
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("[::1]"));
    }

    fn tool_payload(response: &Value) -> Value {
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text result");
        serde_json::from_str(text).expect("tool payload")
    }

    fn temp_data_dir(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "time-keep-mcp-{label}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        path
    }
}
