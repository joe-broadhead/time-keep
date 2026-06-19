# Output Contracts

## CLI Output

JSON is the default and should be treated as the source-of-truth contract.
Table output is for humans. CSV output is for exports.

```bash
time-keep now --tz UTC
time-keep now --tz UTC --table
time-keep calendar 2026-06-18 --output csv
```

## Structured Errors

JSON errors are written to stderr and include:

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

CSV errors use `error_code,message`.

## MCP Output

MCP tools return JSON text content on success. Tool failures are surfaced as MCP
tool errors with `isError: true`, and the text content is the JSON error
envelope.

## Evidence Standard

When reporting a result, preserve:

- command or tool used
- timezone input and resolved timezone
- date range
- whether holidays were included
- timer data directory when timers were mutated
- error details when a call failed
