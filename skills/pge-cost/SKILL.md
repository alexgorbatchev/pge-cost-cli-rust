---
name: pge-cost
description: Estimate continuous device energy costs and fetch PG&E residential rates with pge-cost.
created_on: 2026-10-14 15:57
last_modified: 2026-10-14 15:57
status: current
---

# pge-cost

Use `pge-cost --watts WATTS --plan PLAN` or `pge-cost cost estimate --watts WATTS --plan PLAN` to estimate a device's continuous 24/7 energy cost. Calculations read local data and make no network requests.

## Calculation options

Root and `pge-cost cost estimate` share these options:

- `--watts`: float64, required; finite and greater than zero. Numeric overflow returns an error.
- `--plan`: string, required; case-insensitive, with surrounding whitespace removed. Canonical plans are `E-1`, `E-TOU-C`, `E-TOU-D`, `EV2`, `E-ELEC`, `EV-B`. Aliases: E1; ETOUC, TOUC, TOU-C; ETOUD, TOUD, TOU-D; EV2A, EV2-A, EV-2A; EELEC, ELEC; EVB.
- `--tier`: int32, default 2; E-1 accepts 1 or 2. Other plans ignore this option.
- `--year`: int32, default 2026; accepts 1 through 9999. This selects calendar weights, rather than historical tariffs. TOU calculations reject the fractional Pacific offset transition in 1883.
- `--db`: path, default `rates.json` relative to the working directory. Missing files select bundled rates dated 2026-06-03; unreadable files and malformed JSON return errors.

Human output shows specification and cost tables plus the rates source/date. With `AGENT=1`, `true`, or `yes` (case-insensitive), stdout contains one compact JSON object and LF: `plan_id`, `plan_name`, `watts`, `effective_rate` ($/kWh), `daily_energy_kwh`, `daily_cost`, `monthly_cost`, `annual_cost`, `annual_hours`, `year`, `database`, `embedded`, and `last_updated`. `database` is `embedded` or the displayed file path. Monthly cost is annual cost divided by 12; annual cost includes leap days. Daily cost uses 24 hours at the annual weighted rate.

TOU weights count actual hourly instants in America/Los_Angeles, including daylight-saving gaps/repeated hours. Season bounds are inclusive and must cover each day exactly once. Day schedules must cover all 24 hours exactly once. E-1 uses the selected marginal tier. EV-B applies the tariff's one-hour transition-week adjustment.

Holiday dates preserve the source policy: New Year's Day, Presidents Day, Memorial Day, Independence Day, Labor Day, Veterans Day, Thanksgiving, Christmas; fixed Saturday holidays move to Friday and Sunday holidays to Monday. Observed New Year's dates crossing into an adjacent year are included. PG&E residential tariffs say legally observed dates; the Friday rule is retained from the Go implementation and is not independently confirmed against a published residential holiday calendar.

Estimates use the selected snapshot's total bundled energy rates throughout the chosen year. Fixed charges, credits, taxes, baseline allocations, CARE/FERA discounts, and actual household tier transitions are outside the marginal-device estimate.

## Rate updates

`pge-cost rates fetch` downloads the workbook, parses all six plans using bundled season/schedule definitions, validates every result, and atomically replaces the destination JSON. Existing destination contents survive failed HTTP, parsing, validation, or cancellation before replacement. The downloader has a 90-second deadline and disables automatic retries; Ctrl-C cancels it. Network fetches follow normal HTTP redirects and system proxy settings.

- `--db`: path, default `rates.json`; destination JSON. Parent directories are created. Staging files are private (0600 on Unix); replacement preserves the staging file's permissions.
- `--url`: URL value, default `https://www.pge.com/assets/rates/tariffs/res-inclu-tou-current.xlsx`; absolute HTTP/HTTPS without embedded credentials. The workbook needs one active `Res Inclu TOU_...-Present` worksheet and one `ElecVehicle&Tech_...-Present` worksheet. Missing or duplicate rate sections and nonpositive/nonfinite rate cells return errors. `last_updated` is the UTC fetch date, not the tariff's effective date.

Human stdout contains destination, update date, and a six-plan rate summary. Agent stdout contains one JSON object with `output`, `last_updated`, and the full `plans` map. Requests are read-only; fetching changes the local destination file.

## Metadata and help

Root `--version`, `-v`: bool, default false; prints raw version plus LF. `pge-cost skill` prints the Skills CLI install command and canonical source URL, identically in both modes, offline without files. The Rust migration is unpublished; the canonical source repository's skill and releases may describe Go.

All commands accept `--help`, `-h`: bool, prints command help. Groups are `pge-cost cost` and `pge-cost rates`. Agent help starts with the skill-reading alert. Human help and tables use `COLUMNS`/terminal width; displayed home paths fold to `~`.

Native generated help accepts `[COMMAND]...` after `pge-cost help`: paths include `pge-cost help cost`, `pge-cost help cost estimate`, `pge-cost help rates`, `pge-cost help rates fetch`, `pge-cost help skill`, and `pge-cost help help`. Group paths include `pge-cost cost help`, `pge-cost cost help estimate`, `pge-cost cost help help`, `pge-cost rates help`, `pge-cost rates help fetch`, and `pge-cost rates help help`. There are no completion commands.

Exit 0 means success, including help/version/skill; exit 1 means argument, data, network, filesystem, calculation, or output failure; exit 130 means cancellation. Errors appear once on stderr. Runtime failures have no usage screen. If stdout fails after a rates replacement, exit 1 does not undo the saved database.

## Examples

```sh
pge-cost cost estimate --watts 150 --plan E-TOU-C
AGENT=1 pge-cost --watts 1000 --plan E1 --tier 2 --year 2028
pge-cost rates fetch --db rates.json
pge-cost cost estimate --watts 150 --plan EV2 --db rates.json
```
