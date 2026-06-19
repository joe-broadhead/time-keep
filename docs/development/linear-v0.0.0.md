# Linear v0.0.0 Plan

Project: `time-keep`

Milestone: `v0.0.0 - Production-ready bootstrap`

## Issue Map

| Issue | Title | Release role |
| --- | --- | --- |
| JOE-166 | Lock product contract, repo conventions, and release policy | Confirm scope, conventions, and release policy before scaffold work. |
| JOE-167 | Scaffold Rust crate, CLI shell, paths, output, and errors | Establish buildable crate and shared CLI/error/output foundation. |
| JOE-168 | Implement current time and timezone core | Implement current-time and timezone contracts. |
| JOE-169 | Implement calendar, date arithmetic, diff, and formatting | Implement deterministic date operations. |
| JOE-170 | Implement bounded offline holidays and business days | Implement holiday and business-day behavior. |
| JOE-171 | Implement SQLite timers with migrations and tag filtering | Implement local persistence. |
| JOE-172 | Implement MCP stdio and streamable HTTP server | Expose all tool contracts to agents. |
| JOE-173 | Add integration, protocol, and edge-case test suite | Verify CLI, MCP, persistence, and edge cases. |
| JOE-175 | Write README, MkDocs docs, agent skills, and installer | Complete user and agent documentation plus installer. |
| JOE-176 | Add CI, release workflows, deny/audit policy, SBOM, and provenance | Complete automation and supply-chain gates. |
| JOE-177 | Full production readiness review and audit | Final release gate. |
| JOE-174 | Cut v0.0.0 release from master and verify install/MCP smoke | Publish and verify the first release. |

## Dependency Graph

- JOE-167 blocks JOE-168, JOE-169, JOE-170, JOE-171, JOE-172, and JOE-176.
- JOE-168 through JOE-172 block JOE-173 and JOE-175.
- JOE-173, JOE-175, and JOE-176 block JOE-177.
- JOE-177 blocks JOE-174.

## Commit Policy

Use one commit per issue. Each issue commit should include:

- The Linear issue id in the commit subject.
- Implementation and documentation aligned with the issue acceptance criteria.
- Validation output recorded in Linear.
- Codex autoreview run before commit.
