# Quickstart

## Current Time

```bash
time-keep now --tz UTC --tz Europe/Madrid
```

JSON is the default output. Use `--table` when reading directly in a terminal:

```bash
time-keep now --tz UTC --tz Europe/Madrid --table
```

## Timezone Conversion

```bash
time-keep convert 2026-06-18T12:00:00Z --from UTC --to Europe/Madrid
time-keep tz info Europe/London
time-keep tz list --region europe
```

Use IANA names such as `Europe/London`, `America/New_York`, or `UTC`. Bare city
names such as `Madrid` are rejected with structured `INVALID_PARAMS` errors.

## Calendar And Date Work

```bash
time-keep calendar 2026-06-18
time-keep calc add 2026-01-31 1 month
time-keep calc subtract 2026-06-18 2 weeks
time-keep calc diff 2026-06-01 2026-06-18
time-keep format 2026-06-18T12:00:00Z --output-format rfc2822
```

Use absolute dates in agent prompts and docs when the current date would
otherwise matter.

## Holidays And Business Days

```bash
time-keep holiday check 2026-12-25 --country US
time-keep holiday list 2026 --country GB
time-keep biz between 2026-12-24 2026-12-28 --country US
time-keep biz between 2026-12-24 2026-12-28 --country US --skip-holidays
time-keep biz next 2026-12-25 --country US
```

Holiday data is offline and bounded to `2000..=2030`. Requests outside that
range fail clearly instead of calling a live holiday service.

## Timers

Use `TIME_KEEP_DATA_DIR` for isolated agent runs and tests:

```bash
data_dir="$(mktemp -d)"
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer set q3-planning 2026-07-01T17:00:00-04:00 --tag work --tag planning
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer list --tag work
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer check
```

Timers are stored in local SQLite with deadlines normalized to UTC.

## MCP Server

```bash
time-keep server start --transport stdio
time-keep server start --transport streamable-http --http-host 127.0.0.1 --http-port 8769 --http-path /mcp
```

Streamable HTTP serves JSON-RPC `POST /mcp` plus `GET /healthz` and
`GET /readyz`. Keep HTTP on loopback unless an authenticating proxy controls
access.
