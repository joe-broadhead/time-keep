# Calendar And Date Operations

## Calendar Query

```bash
time-keep calendar 2026-06-18
```

Returns weekday, ISO week/year, day of year, days in month, leap-year state, and
quarter.

## Date Arithmetic

```bash
time-keep calc add 2026-01-31 1 month
time-keep calc subtract 2026-06-18 2 weeks
```

Supported units are seconds, minutes, hours, days, weeks, months, and years.
Month and year shifts clamp to the last valid day when needed, so
`2026-01-31 + 1 month` becomes `2026-02-28` and reports
`month_end_clamped: true`.

## Date Difference

```bash
time-keep calc diff 2026-06-01 2026-06-18
```

Diff output includes signed seconds, minutes, hours, days, weeks, direction,
and absolute seconds.

## Format

```bash
time-keep format 2026-06-18T12:00:00Z --output-format rfc2822
time-keep format 2026-06-18T12:00:00 --output-format epoch
time-keep format 2026-06-18 --output-format strftime --strftime "%A %Y-%m-%d"
```

Timezone-less datetimes default to UTC for epoch, RFC3339, and RFC2822 output.

## Timezones

```bash
time-keep now --tz UTC --tz Europe/Madrid
time-keep tz info Europe/London
time-keep convert 2026-06-18T12:00:00Z --from UTC --to Europe/Madrid
```

IANA timezone names are required. Ambiguous fall-back local times and
nonexistent spring-forward local times fail with `INVALID_PARAMS` rather than
guessing.

## Holidays And Business Days

Holiday data is offline and bounded to `2000..=2030`.

```bash
time-keep holiday check 2026-12-25 --country US
time-keep holiday list 2026 --country GB
time-keep biz between 2026-12-24 2026-12-28 --country US --skip-holidays
```

`biz between` uses inclusive endpoints. Without `--skip-holidays`, it counts
weekdays only. With `--skip-holidays`, `--country` is required. `biz next` and
`biz prev` use strict after/before semantics.
