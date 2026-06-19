# Contributing

The root
[CONTRIBUTING.md](https://github.com/joe-broadhead/time-keep/blob/master/CONTRIBUTING.md)
is the source of truth for development setup, local quality gates, review
expectations, and release-note rules.

For day-to-day work, keep these invariants in view:

- JSON remains the default and source-of-truth output contract.
- IANA timezone names are required for timezone-aware operations.
- Timer persistence stays local, private, and SQLite-backed.
- Holiday and holiday-aware business-day behavior stays offline and bounded.
- MCP stdio and streamable HTTP stay aligned with the CLI contracts.
- Release candidates are cut from `master` only after the full quality gate.

See also:

- [Repo Conventions](repo-conventions.md)
- [Release Policy](release-policy.md)
- [Production Readiness Audit](production-readiness-audit-v0.0.0.md)
