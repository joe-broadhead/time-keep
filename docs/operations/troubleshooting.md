# Troubleshooting

## Invalid Timezone

Use IANA timezone names:

```bash
time-keep tz info Europe/Madrid
```

Bare city names such as `Madrid` return `INVALID_PARAMS`.

## Holiday Year Outside Coverage

Holiday data is bounded to `2000..=2030`:

```bash
time-keep holiday list 2031 --country US
```

The error details include `coverage_start_year`, `coverage_end_year`, and
`runtime_network: false`.

## Timer Data Isolation

If an agent should not touch the default timer database, pass a data directory:

```bash
TIME_KEEP_DATA_DIR="$(mktemp -d)" time-keep timer list
```

For MCP stdio, include `--data-dir` before `server`:

```bash
time-keep --data-dir /tmp/time-keep-agent server start --transport stdio
```

## MCP HTTP Not Reachable

Check readiness:

```bash
curl http://127.0.0.1:8769/healthz
curl http://127.0.0.1:8769/readyz
```

`GET /mcp` returns `405` in v0.0.0. Send JSON-RPC with `POST /mcp`.

## Installer Cannot Resolve Latest

Use an explicit release tag:

```bash
TIME_KEEP_VERSION=v0.0.0 scripts/install.sh --dry-run
```

For private repos or rate limits, set `TIME_KEEP_GITHUB_TOKEN`,
`GITHUB_TOKEN`, or `GH_TOKEN`.
