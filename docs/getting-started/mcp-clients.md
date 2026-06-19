# MCP Clients

## Stdio Client Config

Use stdio for local desktop clients and agent runtimes that launch tools as
child processes:

```json
{
  "mcpServers": {
    "time-keep": {
      "command": "time-keep",
      "args": ["server", "start", "--transport", "stdio"]
    }
  }
}
```

## Streamable HTTP

Use streamable HTTP when the client expects an HTTP endpoint:

```bash
time-keep server start \
  --transport streamable-http \
  --http-host 127.0.0.1 \
  --http-port 8769 \
  --http-path /mcp
```

Health probes:

```bash
curl http://127.0.0.1:8769/healthz
curl http://127.0.0.1:8769/readyz
```

JSON-RPC requests are accepted with `POST /mcp`. Streaming `GET /mcp` returns
`405 Method Not Allowed` in v0.0.0.

## Data Isolation

For disposable agent runs, pass an isolated data directory:

```json
{
  "mcpServers": {
    "time-keep": {
      "command": "time-keep",
      "args": ["--data-dir", "/tmp/time-keep-agent", "server", "start", "--transport", "stdio"]
    }
  }
}
```

This keeps timer state out of the user's default
`~/.local/share/time-keep/timers.db`.
