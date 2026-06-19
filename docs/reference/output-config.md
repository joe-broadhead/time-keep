# Output, Config, And Paths

## Output Modes

JSON is the default:

```bash
time-keep calendar 2026-06-18
```

Table output is for human inspection:

```bash
time-keep calendar 2026-06-18 --table
```

CSV output is for spreadsheets and simple exports:

```bash
time-keep calendar 2026-06-18 --output csv
```

Scripts should prefer JSON when nested fields or application/runtime errors
matter.

## Structured Errors

For application/runtime failures, stdout remains empty and stderr receives the
selected error format. JSON errors include `error.error_code`, `error.message`,
and optional `error.details`.

Argument parsing and usage failures are emitted by the CLI parser before command
execution, so they use standard plain-text usage diagnostics.

## Config Path

```bash
time-keep config path
```

Default config path:

```text
XDG_CONFIG_HOME/time-keep/config.toml
```

Fallback:

```text
~/.config/time-keep/config.toml
```

The v0.0.0 CLI resolves the path but does not require a config file for normal
operation.

## Data Path

Default data directory:

```text
XDG_DATA_HOME/time-keep
```

Fallback:

```text
~/.local/share/time-keep
```

Override with:

```bash
time-keep --data-dir /tmp/time-keep-data timer list
TIME_KEEP_DATA_DIR=/tmp/time-keep-data time-keep timer list
```

Only timers persist in v0.0.0.
