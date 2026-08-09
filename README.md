# time-keep

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.93%2B-orange.svg?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Docs](https://img.shields.io/badge/docs-mkdocs%20material-blue.svg?logo=materialformkdocs&logoColor=white)](https://joe-broadhead.github.io/time-keep/)
[![CI](https://github.com/joe-broadhead/time-keep/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/joe-broadhead/time-keep/actions/workflows/ci.yml)

<pre>
   __  _                      __
  / /_(_)___ ___  ___        / /_____  ___  ____
 / __/ / __ `__ \/ _ \______/ //_/ _ \/ _ \/ __ \
/ /_/ / / / / / /  __/_____/ ,< /  __/  __/ /_/ /
\__/_/_/ /_/ /_/\___/     /_/|_|\___/\___/ .___/
                                        /_/
             Clock context
           for planning agents.
</pre>

`time-keep` is a **local-first agent clock CLI and MCP server** for reliable
time context: current time, IANA timezone operations, calendar queries,
date arithmetic, business days, local SQLite timers, and bounded offline
holiday lookups.

It is built for agents and scripts that need stable outputs and explicit
time assumptions. JSON is the default output, table and CSV are available when
useful, and runtime behavior does not call network time APIs or live holiday
APIs.

## What It Does

- **Shows current time** in UTC and requested IANA timezones.
- **Lists and inspects IANA timezones**, including offset, DST state, and next
  transition metadata when available.
- **Converts datetimes** between IANA zones and rejects ambiguous or nonexistent
  local times.
- **Runs calendar queries** for week number, day of year, month length, leap
  year, and quarter.
- **Performs date arithmetic, diffs, and formatting** with stable timezone
  rules.
- **Checks and lists offline holidays** with explicit `2000..=2030` coverage.
- **Counts weekend-only or holiday-aware business days** for deterministic
  planning flows.
- **Stores named timers** in local SQLite with WAL, migrations, private file
  modes, and normalized tag filtering.
- **Exposes 15 MCP tools** over stdio and streamable HTTP.

## 30-Second Example

```bash
time-keep now --tz UTC --tz Europe/Madrid
```

Example output shape (abridged):

```json
{
  "generated_at_utc": "2026-06-19T05:32:16Z",
  "format": "rfc3339",
  "times": [
    {
      "timezone": "UTC",
      "utc_offset_seconds": 0,
      "is_dst": false
    },
    {
      "timezone": "Europe/Madrid",
      "utc_offset_seconds": 7200,
      "is_dst": true
    }
  ]
}
```

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/time-keep/HEAD/scripts/install.sh | bash
```

Install the binary and time-keep agent skills:

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/time-keep/HEAD/scripts/install.sh | bash -s -- --install-skills
```

Install shell completions explicitly:

```bash
curl -fsSL https://raw.githubusercontent.com/joe-broadhead/time-keep/HEAD/scripts/install.sh | bash -s -- --install-completions --shell zsh
```

The installer downloads release assets, verifies SHA-256 checksums by default,
and can install skills from `.github/skills`. Preview its choices without
network downloads:

```bash
scripts/install.sh --dry-run
```

### Prebuilt Binaries

Release assets are published from the GitHub Release workflow with checksums,
SBOMs, and provenance JSON assets.

```bash
# macOS Apple Silicon, using an authenticated GitHub CLI session.
gh release download \
  --repo joe-broadhead/time-keep \
  -p time-keep-macos-arm64.tar.gz \
  -p time-keep-macos-arm64.sha256

shasum -a 256 -c time-keep-macos-arm64.sha256
tar -xzf time-keep-macos-arm64.tar.gz
./time-keep-macos-arm64/time-keep --version
```

Choose the asset for your platform from
[Releases](https://github.com/joe-broadhead/time-keep/releases).

### From Source

```bash
git clone https://github.com/joe-broadhead/time-keep.git
cd time-keep
cargo build --locked --release

./target/release/time-keep now --tz UTC --tz Europe/Madrid
```

## Quick Start

```bash
time-keep now --tz UTC --tz Europe/Madrid
time-keep tz info Europe/London
time-keep tz list --region europe
time-keep convert 2026-06-18T12:00:00Z --from UTC --to Europe/Madrid
time-keep format 2026-06-18T12:00:00Z --output-format rfc2822
time-keep calc add 2026-01-31 1 month
time-keep calc diff 2026-06-01 2026-06-18
time-keep calendar 2026-06-18
time-keep holiday check 2026-12-25 --country US
time-keep biz between 2026-12-24 2026-12-28 --country US --skip-holidays
```

## CLI Usage

```bash
time-keep now [--tz UTC] [--tz Europe/Madrid]
time-keep tz list [--region europe]
time-keep tz info Europe/London
time-keep convert 2026-06-18T12:00:00Z --from UTC --to Europe/Madrid
time-keep calendar 2026-06-18
time-keep calc add 2026-01-31 1 month
time-keep calc subtract 2026-06-18 2 weeks
time-keep calc diff 2026-06-01 2026-06-18
time-keep format 2026-06-18T12:00:00Z --output-format rfc2822
time-keep holiday check 2026-12-25 --country US
time-keep holiday list 2026 --country GB
time-keep biz between 2026-12-24 2026-12-28 --country US --skip-holidays
time-keep biz next 2026-12-25 --country US
time-keep biz prev 2026-12-25 --country US
time-keep timer set <name> <deadline> [--description text] [--tag tag]
time-keep timer get <name>
time-keep timer list [--tag tag]
time-keep timer check
time-keep timer delete <name>
time-keep server start --transport stdio
time-keep server start --transport streamable-http --http-port 8769
time-keep completions zsh
time-keep config path
```

Global options:

```bash
--output json|table|csv
--table
--config <path>
--data-dir <path>
```

## Default Timezone

With no configured default, `now` and the `current_time` MCP tool report UTC
when no timezone is given. To change that default, set it in `config.toml`:

```toml
default_timezone = "Europe/Madrid"

# Or an ordered list, which takes precedence over the singular form:
# default_timezones = ["Europe/Madrid", "UTC"]

# Or detect the operating-system timezone cross-platform:
# default_timezone = "system"
```

Resolution precedence (highest first): explicit `--tz` flags, then the
`TIME_KEEP_TZ` environment variable (single name, comma-separated list, or
`system`), then `config.toml`, then UTC.

```bash
TIME_KEEP_TZ=Europe/Madrid time-keep now
```

See [Output, Config, And Paths](docs/reference/output-config.md) for details.

## Timers

Timers persist locally. Use `TIME_KEEP_DATA_DIR` for isolated tests and agent
runs:

```bash
data_dir="$(mktemp -d)"
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer set q3-planning 2026-07-01T17:00:00-04:00 --description "Q3 planning due" --tag work --tag planning
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer get q3-planning
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer list --tag work
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer check
TIME_KEEP_DATA_DIR="$data_dir" time-keep timer delete q3-planning
```

Default database path:

```text
XDG_DATA_HOME/time-keep/timers.db
```

Fallback path:

```text
~/.local/share/time-keep/timers.db
```

## Offline Holiday Coverage

Holiday and holiday-aware business-day results come from generated offline
data. Holiday coverage is explicitly bounded to years `2000..=2030`.
Requests outside that range return structured `INVALID_PARAMS` errors with the
supported coverage in `details`.

`biz between` uses inclusive endpoints. It is weekend-only unless
`--skip-holidays` is passed with `--country`. `biz next` and `biz prev` use
strict after/before semantics and skip country holidays when `--country` is
supplied.

## MCP Server

```bash
time-keep server start --transport stdio
time-keep server start --transport streamable-http --http-host 127.0.0.1 --http-port 8769 --http-path /mcp
```

Streamable HTTP serves JSON-RPC `POST /mcp`, plus `GET /healthz` and
`GET /readyz`. It binds to `127.0.0.1` by default and warns on non-loopback
binds. Keep it on loopback unless an authenticating proxy controls access.

The MCP surface exposes:

```text
current_time, list_timezones, timezone_info, convert_timezone,
calendar_query, date_arithmetic, date_diff, date_format,
holidays, business_days,
timer_set, timer_get, timer_list, timer_delete, timer_check
```

## Output And Errors

Global output options:

```bash
--output json|table|csv
--table
```

JSON is the default and source-of-truth contract. Application/runtime errors
leave stdout empty and write the selected error format to stderr. JSON errors
include `error.error_code`, `error.message`, and optional `error.details`.
Argument parsing and usage failures use standard plain-text CLI diagnostics.

## Documentation

- [Quickstart](docs/getting-started/quickstart.md)
- [Installation](docs/getting-started/installation.md)
- [CLI Reference](docs/reference/cli.md)
- [MCP Reference](docs/reference/mcp.md)
- [Timers](docs/reference/timers.md)
- [Security](docs/operations/security.md)
- [Agent Skills](docs/development/agent-skills.md)
- [Agent Development Guide](AGENTS.md)
- [Contributing](CONTRIBUTING.md)

## Release Policy

Releases are cut from `master`. Release candidates must pass CI, docs,
installer checks, release automation, supply-chain checks, install smoke, and
MCP smoke before a tag is published.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, quality gates,
documentation checks, and review expectations.

## License

MIT. See [LICENSE](LICENSE).
