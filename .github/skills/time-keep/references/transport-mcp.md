# MCP Transport

Prefer MCP when time-keep tools are available in the client. MCP tool results
return JSON text content matching the CLI response contract where practical.

## Tool Selection

| Need | Tool |
| --- | --- |
| Current time | `current_time` |
| List IANA zones | `list_timezones` |
| Inspect timezone | `timezone_info` |
| Convert timezone | `convert_timezone` |
| Calendar fields | `calendar_query` |
| Date add/subtract | `date_arithmetic` |
| Date diff | `date_diff` |
| Parse/format dates | `date_format` |
| Holiday check/list | `holidays` |
| Business days | `business_days` |
| Set timer | `timer_set` |
| Read timer | `timer_get` |
| List timers | `timer_list` |
| Delete timer | `timer_delete` |
| Check overdue timers | `timer_check` |

## Server Startup

For local MCP clients:

```bash
time-keep server start --transport stdio
```

For HTTP clients:

```bash
time-keep server start --transport streamable-http --http-host 127.0.0.1 --http-port 8769 --http-path /mcp
```

Health probes:

```bash
curl http://127.0.0.1:8769/healthz
curl http://127.0.0.1:8769/readyz
```

Keep streamable HTTP on loopback unless an authenticating proxy controls
access.

## Common Calls

Current time:

```json
{
  "timezones": ["UTC", "Europe/Madrid"],
  "format": "rfc3339"
}
```

Business days:

```json
{
  "action": "between",
  "from": "2026-12-24",
  "to": "2026-12-28",
  "country": "US",
  "skip_holidays": true
}
```

Timer:

```json
{
  "name": "q3-planning",
  "deadline": "2026-07-01T17:00:00-04:00",
  "tags": ["work", "planning"]
}
```

## Error Handling

MCP tool failures are surfaced as tool errors (`isError: true`). Read the JSON
error text, correct the offending argument, and retry only when appropriate.
