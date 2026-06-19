# Production Readiness Audit: v0.0.0

Date: 2026-06-19

Linear gate: JOE-177

Commit under review: `4911911`

Decision: pass for JOE-174 release execution. This pre-release audit allowed
the `v0.0.0` tag only after JOE-177 was complete in Linear.

## Scope

This audit covers the v0.0.0 CLI/MCP contract, SQLite timer persistence,
security posture, supply-chain controls, documentation, installer, and release
automation.

## Results

| Area | Result | Notes |
| --- | --- | --- |
| Blockers | Pass | JOE-173, JOE-175, and JOE-176 are Done. |
| CLI contract | Pass | JSON/table/CSV output and all command families are implemented and covered by tests. |
| MCP contract | Pass | 15 tools are exposed over stdio and streamable HTTP; tool schemas and argument allowlists are tested. |
| Persistence | Pass | SQLite timers use WAL, `busy_timeout=5000`, `foreign_keys=ON`, `PRAGMA user_version`, private Unix file mode, normalized tags, UTC deadlines, and temporary-data tests. |
| Local-only behavior | Pass | Runtime time, calendar, holiday, business-day, and timer paths do not use live network APIs. Installer and release workflows are the only networked paths. |
| HTTP security | Pass | HTTP defaults to loopback, warns on non-loopback binds, applies origin checks, request header/body limits, and health/readiness endpoints. |
| Supply chain | Pass | `cargo deny check` and `cargo audit` pass; `deny.toml` includes advisory, license, source, and `holidays` clarification policy. |
| Release automation | Pass | CI/docs/release workflows are syntax-checked, actions are SHA-pinned, release PRs require `RELEASE_PR_TOKEN`, release tags require `RELEASE_TAG_TOKEN`, and assets publish only after the full platform matrix succeeds. |
| Installer | Pass | File-backed release-asset install smoke verified checksum validation, zsh completions, installed binary execution, timer persistence, MCP stdio, and MCP HTTP. |
| Docs and skills | Pass | MkDocs strict build passes; README, reference docs, operations docs, and standalone skills cover local-first and bounded-holiday behavior. |

## Validation Evidence

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | Pass |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `cargo test --locked --all-features` | Pass: 84 unit tests and 37 integration tests |
| `/tmp/time-keep-docs-venv/bin/mkdocs build --strict` | Pass, with the upstream Material for MkDocs warning about future MkDocs 2.0 changes |
| `actionlint .github/workflows/*.yml` | Pass |
| `bash -n scripts/install.sh` | Pass |
| `shellcheck scripts/install.sh` | Pass |
| `git diff --check` | Pass |
| `cargo deny check` | Pass |
| `cargo audit` | Pass |
| Clean install/MCP smoke | Pass |
| GitHub Docs workflow on `master` | Pass: run `27805031139` |
| GitHub CI workflow on `master` | Pass: run `27805031143` |

## Accepted Risks

- The release publish workflow was not executed during this audit because it
  was intentionally gated by the then-planned `v0.0.0` tag. Static workflow
  validation,
  local release-archive smoke, installer smoke, and remote `master` CI/docs
  runs passed. JOE-174 must verify the actual tag-triggered release artifacts.
- MkDocs emits the Material for MkDocs upstream warning about proposed MkDocs
  2.0 changes. The strict documentation build succeeds, so this is accepted for
  v0.0.0 and should be revisited only if the docs toolchain changes.

## Release Decision

No blocking findings remain. JOE-174 may cut `v0.0.0` from `master`, provided
the release execution records the tag, published assets, checksums, installer
smoke, binary smoke, completions smoke, MCP stdio smoke, and MCP HTTP smoke.
