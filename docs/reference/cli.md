# CLI Reference

Global options:

```bash
--output json|table|csv
--table
--config <path>
--data-dir <path>
```

JSON is the default output and the source-of-truth contract. Diagnostics and
structured errors are written to stderr.

## Commands

```bash
time-keep now [--tz <IANA>]... [--format rfc3339|iso8601|epoch]
time-keep tz info <IANA>
time-keep tz list [--region europe]
time-keep convert <datetime> --from <IANA> --to <IANA>
time-keep format <datetime> [--output-format iso8601|rfc3339|rfc2822|epoch|unix-timestamp|strftime] [--strftime <pattern>] [--input-format <pattern>]
time-keep calc add <date-or-datetime> <amount> <unit>
time-keep calc subtract <date-or-datetime> <amount> <unit>
time-keep calc diff <from> <to>
time-keep calendar <date>
time-keep holiday check <date> --country <ISO2>
time-keep holiday list <year> --country <ISO2>
time-keep biz between <from> <to> [--country <ISO2>] [--skip-holidays]
time-keep biz next <date> [--country <ISO2>]
time-keep biz prev <date> [--country <ISO2>]
time-keep timer set <name> <deadline> [--description <text>] [--tag <tag>]...
time-keep timer get <name>
time-keep timer list [--tag <tag>]
time-keep timer delete <name>
time-keep timer check
time-keep config path
time-keep server start --transport stdio
time-keep server start --transport streamable-http [--http-host 127.0.0.1] [--http-port 8769] [--http-path /mcp]
time-keep completions zsh|bash|fish|powershell|elvish
```

## Error Contract

JSON errors use:

```json
{
  "error": {
    "error_code": "INVALID_PARAMS",
    "message": "invalid IANA timezone name: Madrid",
    "details": {
      "parameter": "timezone",
      "value": "Madrid"
    }
  }
}
```

CSV errors use `error_code,message`. Table errors use `Error Code` and
`Message` columns.
