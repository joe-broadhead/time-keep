# CLI Transport

## Check Installation

```bash
time-keep --version
time-keep --help
```

If the binary is not installed but the repo is available:

```bash
cargo run -- now --tz UTC
```

Use the release binary for production smoke tests:

```bash
cargo build --locked --release
./target/release/time-keep --version
./target/release/time-keep now --tz UTC
```

## Time And Timezones

```bash
time-keep now --tz UTC --tz Europe/Madrid
time-keep tz info Europe/London
time-keep tz list --region europe
time-keep convert 2026-06-18T12:00:00Z --from UTC --to Europe/Madrid
```

Use IANA names only.

## Calendar And Dates

```bash
time-keep calendar 2026-06-18
time-keep calc add 2026-01-31 1 month
time-keep calc subtract 2026-06-18 2 weeks
time-keep calc diff 2026-06-01 2026-06-18
time-keep format 2026-06-18T12:00:00Z --output-format rfc2822
```

Prefer absolute dates in prompts and reports.

## Holidays And Business Days

```bash
time-keep holiday check 2026-12-25 --country US
time-keep holiday list 2026 --country GB
time-keep biz between 2026-12-24 2026-12-28 --country US --skip-holidays
time-keep biz next 2026-12-25 --country US
```

Holiday data is offline and bounded to `2000..=2030`.

## Timers

```bash
data_dir="$(mktemp -d)"
TIME_KEEP_DATA_DIR="${data_dir}" time-keep timer set q3-planning 2026-07-01T17:00:00-04:00 --tag work
TIME_KEEP_DATA_DIR="${data_dir}" time-keep timer list --tag work
TIME_KEEP_DATA_DIR="${data_dir}" time-keep timer check
```

Use isolated data directories for tests and temporary agent runs.

## Output Modes

```bash
time-keep calendar 2026-06-18
time-keep calendar 2026-06-18 --table
time-keep calendar 2026-06-18 --output csv
```

Logs and diagnostics go to stderr. Machine-readable command output stays on
stdout.
