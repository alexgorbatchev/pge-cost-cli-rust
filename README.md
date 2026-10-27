`pge-cost` estimates the marginal electricity cost of running a device continuously on six PG&E residential plans. It combines a rate snapshot with calendar scheduling and can refresh local rates from PG&E's official workbook.

# What It Does

- **Estimate device costs:** Calculate daily, average monthly, and annual energy costs from wattage and a rate plan.
- **Account for the calendar:** Include leap days, Pacific daylight-saving hours, seasons, and the configured holiday policy.
- **Refresh local rates:** Download and validate all six plans before replacing the rates file.
- **Produce automation output:** Use `AGENT=1` for compact JSON results.

# How It Works

- Supply device wattage and choose E-1, E-TOU-C, E-TOU-D, EV2, E-ELEC, or EV-B.
- Read `rates.json` or select the bundled 2026-06-03 snapshot when that file is absent.
- Weight the selected plan's rates over the chosen calendar and report energy consumption and costs.
- Run `pge-cost rates fetch` to refresh the local snapshot from the published workbook.

# How it Really Works

- Calculations run offline. Only `rates fetch` makes an HTTP request; it has a 90-second deadline and supports Ctrl-C cancellation.
- Rates remain constant within their configured seasons throughout the requested year. Changing `--year` changes calendar weights, rather than selecting historical rates.
- Annual totals include leap days; monthly totals equal annual totals divided by 12. Daily totals represent 24 hours at the annual weighted rate.
- TOU schedules count actual Pacific hours. E-1 uses the selected marginal tier; fixed charges, credits, taxes, discounts, baseline allocations, and household tier transitions are outside this estimate.
- Holiday observation retains the source policy: Saturday fixed holidays move to Friday and Sunday holidays to Monday. The residential tariff specifies legally observed dates; an independent residential calendar confirming the Friday rule remains unavailable.
- Invalid or unreadable local databases return errors. Workbook failures and cancellation before replacement preserve the previous destination; successful replacements use private staging files, with 0600 permissions on Unix.
- Human mode prints tables; `AGENT=1`, `true`, or `yes` prints JSON. Results go to stdout and errors appear once on stderr. Exit codes are 0 for success, 1 for failure, and 130 for cancellation.

# Installation

This Rust implementation is local and has no published prebuilt release. The canonical [source repository](https://github.com/alexgorbatchev/pge-cost-cli) may contain Go artifacts; no Rust download URL is available.

# Quick Start

```sh
pge-cost cost estimate --watts 150 --plan E-TOU-C
pge-cost rates fetch --db rates.json
pge-cost cost estimate --watts 1000 --plan E1 --tier 2 --year 2028
AGENT=1 pge-cost cost estimate --watts 150 --plan E1 --db device-rates.json
```

Sample Output when `device-rates.json` is absent:

```json
{"annual_cost":534.82428,"annual_hours":8760,"daily_cost":1.465272,"daily_energy_kwh":3.6,"database":"embedded","effective_rate":0.40702,"embedded":true,"last_updated":"2026-06-03","monthly_cost":44.568690000000004,"plan_id":"E-1","plan_name":"Residential Services (Tiered)","watts":150.0,"year":2026}
```

# Options & Flags

Root metadata options:

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--version` | `-v` | `false` | Print the raw version and exit |
| `--help` | `-h` | `false` | Print help for any command |

`pge-cost cost estimate` and root calculation flags:

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--watts <WATTS>` | — | Required | Finite positive continuous wattage |
| `--plan <PLAN>` | — | Required | Supported plan or billing alias |
| `--tier <TIER>` | — | `2` | E-1 tier, 1 or 2; ignored for other plans |
| `--year <YEAR>` | — | `2026` | Calendar year from 1 through 9999; TOU rejects the fractional historical clock transition in 1883 |
| `--db <PATH>` | — | `rates.json` | Local rate snapshot; missing files select bundled rates |

`pge-cost rates fetch`:

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--db <PATH>` | — | `rates.json` | Destination JSON file |
| `--url <URL>` | — | `https://www.pge.com/assets/rates/tariffs/res-inclu-tou-current.xlsx` | HTTP/HTTPS workbook source without embedded credentials |

`pge-cost skill` prints the canonical skill installation pointer. The complete local usage contract is [skills/pge-cost/SKILL.md](skills/pge-cost/SKILL.md).

# License

[MIT License](LICENSE), copyright 2026 Alex Gorbatchev.
