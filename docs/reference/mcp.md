# MCP Reference

time-keep exposes the same local time workflows through MCP for agents that
prefer tools over shell commands.

For client configuration examples, see [MCP Clients](../getting-started/mcp-clients.md).

## Stdio

```bash
time-keep server start --transport stdio
```

Stdio keeps stdout reserved for newline-delimited JSON-RPC protocol messages.
Diagnostics go to stderr.

## Streamable HTTP

```bash
time-keep server start \
  --transport streamable-http \
  --http-host 127.0.0.1 \
  --http-port 8769 \
  --http-path /mcp
```

Health probes:

```bash
curl http://127.0.0.1:8769/healthz
curl http://127.0.0.1:8769/readyz
```

Streamable HTTP is stateless in v0.0.0. `POST /mcp` accepts JSON-RPC requests.
`GET /mcp` returns `405 Method Not Allowed` until a future issue intentionally
adds streaming reads.

Keep HTTP bound to loopback unless an authenticating proxy controls access. The
server warns on non-loopback binds.

## Tools

| Tool | Use |
| --- | --- |
| `current_time` | Current datetime in UTC and requested IANA zones |
| `list_timezones` | List IANA names with optional region filtering |
| `timezone_info` | Current offset, DST state, abbreviation, and next transition |
| `convert_timezone` | Convert a datetime between IANA zones |
| `calendar_query` | Week, day-of-year, days-in-month, leap year, and quarter |
| `date_arithmetic` | Add or subtract seconds, minutes, hours, days, weeks, months, or years |
| `date_diff` | Difference between two dates or datetimes |
| `date_format` | Parse and format a date or datetime |
| `holidays` | Check one holiday or list holidays for a country/year |
| `business_days` | Count business days or find next/previous business day |
| `timer_set` | Create or update a local SQLite timer |
| `timer_get` | Read one timer |
| `timer_list` | List timers with optional tag filtering |
| `timer_delete` | Delete one timer |
| `timer_check` | List overdue timers |

## Error Handling

Tool failures are returned as MCP tool errors with `isError: true`. The text
content is a JSON error envelope matching the CLI structured error contract.

Retry only after correcting the offending parameter. Examples include invalid
IANA timezone names, missing required tool arguments, years outside holiday
coverage, or a missing timer name.
