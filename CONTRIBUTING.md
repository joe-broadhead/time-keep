# Contributing

Thanks for helping keep `time-keep` boring, deterministic, and useful for
agents. The project is a local-first Rust CLI and MCP server, so changes should
preserve stable machine-readable output, explicit timezone behavior, and safe
local persistence.

## Development Setup

```bash
git clone https://github.com/joe-broadhead/time-keep.git
cd time-keep
cargo build --locked
```

Use temporary data directories for manual timer checks:

```bash
data_dir="$(mktemp -d)"
TIME_KEEP_DATA_DIR="$data_dir" cargo run -- timer set demo 2026-07-01T12:00:00Z
TIME_KEEP_DATA_DIR="$data_dir" cargo run -- timer list
```

## Quality Gates

Run the full local gate before opening a PR or cutting a release candidate:

```bash
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
python3 -m mkdocs build --strict
cargo deny check
cargo audit
```

When workflow files change, run `actionlint .github/workflows/*.yml` if
available. When shell scripts change, run `bash -n` and `shellcheck` if
available.

Run the Codex autoreview gate before committing:

```bash
codex review --uncommitted
```

## Review Expectations

- Keep each change scoped to one coherent issue or release task.
- Preserve JSON as the source-of-truth output contract.
- Add tests for behavior changes, edge cases, persistence, and MCP tools.
- Keep runtime behavior local-only: no network time APIs or live holiday APIs.
- Document accepted limits, especially holiday coverage and HTTP exposure.
- Keep README and MkDocs files ASCII-only unless a file already uses Unicode.

## Release Notes

Update `CHANGELOG.md` for user-visible changes. Release tags are cut from
`master` only after CI, docs, supply-chain checks, install smoke, MCP smoke, and
the production readiness review are complete.
