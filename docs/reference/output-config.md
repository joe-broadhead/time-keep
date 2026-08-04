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

The config file is optional. With no config file, behavior is unchanged.

## Default Timezone

By default, `now` and the `current_time` MCP tool report UTC when no timezone is
given. You can set a default timezone (or an ordered list) in `config.toml`:

```toml
# Single default timezone.
default_timezone = "Europe/Amsterdam"

# Or an ordered list (takes precedence over default_timezone when both are set).
default_timezones = ["Europe/Amsterdam", "UTC"]
```

Use the special value `system` (alias `local`) to detect the operating-system
timezone from the `TZ` environment variable or the `/etc/localtime` symlink:

```toml
default_timezone = "system"
```

The default is resolved with the following precedence (highest first):

1. Explicit `--tz` flags on `now` (or the `timezones` argument to
   `current_time`, including an explicit empty list, which means UTC).
2. The `TIME_KEEP_TZ` environment variable. Accepts a single IANA name, a
   comma-separated list, or the `system`/`local` token.
3. `default_timezones`, then `default_timezone`, from `config.toml`.
4. UTC.

`TIME_KEEP_TZ` is consulted before the config file is read, so a valid
environment override works even when the config file is invalid.

Only IANA timezone names are accepted. An invalid name, or a `system` token that
cannot be resolved, produces a structured `INVALID_PARAMS` error rather than a
silent fallback. `TIME_KEEP_TZ` is handy for one-off overrides:

```bash
TIME_KEEP_TZ=Europe/Amsterdam time-keep now
TIME_KEEP_TZ=system time-keep now
```

Scoping and resilience:

- Only the commands that use the default read the config file: `now` without
  `--tz`, and the `current_time` MCP tool without a `timezones` argument. A
  broken config never blocks `config path`, timers, calendar, holiday, or
  business-day commands.
- Unknown config keys are ignored with a warning, so configs written for other
  time-keep versions keep working.
- The MCP server starts even when the configured default is invalid: it prints
  a warning at startup, and only `current_time` calls that rely on the default
  return the structured error.
- Defaults are re-resolved on every call, so a long-running MCP server picks up
  config edits and operating-system timezone changes (the `system` token)
  without a restart.

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
