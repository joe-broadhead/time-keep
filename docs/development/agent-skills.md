# Agent Skills

time-keep includes three standalone agent skills under `.github/skills/`:

```text
.github/skills/time-keep/SKILL.md
.github/skills/time-keep-calendar/SKILL.md
.github/skills/time-keep-timers/SKILL.md
```

## Skills

- `time-keep`: core MCP/CLI usage, transport selection, output contracts, and
  evidence standards.
- `time-keep-calendar`: date arithmetic, timezone conversion, business days,
  and holiday guardrails.
- `time-keep-timers`: local SQLite timer setup, isolation, listing, checking,
  and deletion.

## References

The core skill uses small reference files that agents load only when needed:

- `transport-cli.md`
- `transport-mcp.md`
- `output-contracts.md`

## Typical Agent Prompt

```text
Use time-keep to convert 2026-06-18T12:00:00Z from UTC to Europe/Madrid,
then count US business days from 2026-12-24 through 2026-12-28 with holidays.
```

## Validation

Before publishing a skill update, run:

```bash
cargo test --locked --all-features
mkdocs build --strict
```
