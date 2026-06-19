# Security

time-keep is designed for local agent and automation workflows.

## No Runtime Network Dependencies

v0.0.0 does not call network time APIs or live holiday APIs at runtime. Current
time comes from the local system clock. Holiday and business-day behavior uses
offline bounded data with `2000..=2030` coverage.

## Local Data

Timers are stored in local SQLite:

```text
~/.local/share/time-keep/timers.db
```

Treat timer names, descriptions, deadlines, and tags as private user data. Use
isolated `TIME_KEEP_DATA_DIR` values for CI, shared runners, or disposable
agent sessions.

## MCP HTTP

The streamable HTTP MCP server should stay on loopback unless an authenticating
proxy controls access.

```bash
time-keep server start \
  --transport streamable-http \
  --http-host 127.0.0.1 \
  --http-port 8769 \
  --http-path /mcp
```

Binding to a public interface makes local time and timer tools available to any
client that can reach the server. time-keep does not implement
application-level HTTP authentication in v0.0.0.

## Installer

The installer verifies SHA-256 checksums by default. Keep checksum verification
enabled outside controlled local testing.

## Reporting Issues

Report security issues privately until a public vulnerability reporting channel
is configured for the repository.
