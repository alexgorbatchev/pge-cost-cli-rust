---
created_on: 2026-10-14 15:57
last_modified: 2026-10-14 15:57
status: current
---

# pge-cost

Rust CLI for marginal energy costs of continuously running devices on PG&E residential rate plans. Inherit [../AGENTS.md](../AGENTS.md).

## Commands

- `just run cost estimate --watts 150 --plan E-TOU-C`
- `just run-ai cost estimate --watts 150 --plan E-TOU-C`
- `just test`, `just lint`, `just coverage`, `just check`
- `just build` writes `bin/pge-cost`.

## Calculation contracts

- Keep the imported JSON schema and all six supported plans. Calendar year changes weights, not the historical tariff snapshot.
- Count native Pacific hourly instants; include leap days in annual/monthly cost. Reject overlapping/gapped seasons or schedules and nonfinite/nonpositive used rates. Reject fractional historical timezone transitions instead of rounding to an extra hour.
- Preserve the explicit holiday policy until a verified residential tariff/calendar establishes a change. The Go Friday observation for Saturday holidays is not independently confirmed; PG&E's nonresidential holiday page is not evidence for residential plans.
- Preserve EV-B's tariff transition weeks separately from native Pacific clock transitions. Consult the official schedule EV time-period section.

## File and network contracts

- Use bundled rates only for a missing database; do not hide corrupt or unreadable files behind fallback.
- Parse XLSX with native calamine absolute cell coordinates. Require unique active worksheet prefixes and all six rate sections; do not retain stale rates when a section is absent.
- Clone baseline data before replacement; use validated same-directory staging, file sync, and native atomic persist. Failed downloads/parses leave the destination intact.
- Fetch only with the normal reqwest/system-proxy transport, a 90-second deadline, cancellation, and retries disabled. Reject source URL credentials before networking.
- Keep machine output on stdout and returned errors at the single entry boundary. If output fails after local persistence, do not claim rollback.

## Maintenance

- Change behavioral tests whenever runtime results change; require at least 90% line coverage, excluding `scripts/`. Prove relevant tests fail when disabling the implementation, restore it, and rerun checks.
- Update `skills/pge-cost/SKILL.md` and its `last_modified` metadata in the same change as command, flag, argument, default, environment, output, or side-effect changes. Keep live interface drift coverage.
- Keep temporary evidence in `.tmp/`; scope formatters to this package. Preserve standing instructions in place and task progress outside this file.
- Never publish releases, tags, packages, or production deployments without explicit user authorization. Do not fabricate Rust release URLs or treat Go artifacts as Rust distributions.
