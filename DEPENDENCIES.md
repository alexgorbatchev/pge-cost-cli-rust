---
created_on: 2026-10-14 15:57
last_modified: 2026-10-14 15:57
status: current
---

# Native dependencies

| Go dependency / behavior | Rust dependency |
| :--- | :--- |
| standard flag parser / Cobra-style CLI contracts | Native Clap via local cli-command-kit; no argv parser facade. |
| excelize v2 | calamine 0.36.1, native Xlsx reader and absolute Range coordinates. |
| go-pretty table | comfy-table 8.0.1, dynamic terminal-width rounded tables. |
| time / net/http / JSON | chrono 0.4.45 + chrono-tz 0.10.4, reqwest 0.13.5 native Rustls client, serde + serde_json. |
| fixed temporary download + direct file write | In-memory XLSX bytes and tempfile 3.27 native private atomic persistence. |
| cancellation | Tokio signal handling and tokio-util CancellationToken. |

The direct dependencies use MIT, Apache-2.0, or compatible dual licensing. Cargo.lock pins the resolved graph. rust_xlsxwriter 0.99.1 creates native deterministic test workbooks; tiny_http 0.12 serves actual local HTTP fixtures.

Rate schema and bundled values come from the MIT Go source's `pkg/domain/rates_fallback.json`, dated 2026-06-03. Official workbook: [PG&E residential rates](https://www.pge.com/assets/rates/tariffs/res-inclu-tou-current.xlsx). Schedule timing: [E-TOU-D](https://www.pge.com/tariffs/assets/pdf/tariffbook/ELEC_SCHEDS_E-TOU-D.pdf) and [EV](https://www.pge.com/tariffs/assets/pdf/tariffbook/ELEC_SCHEDS_EV.pdf). Rates change independently of calendar weights.

Verified Go corrections: leap-year annual/monthly totals; same-month season bounds; cross-year New Year's observation; rejecting schedule gaps/overlaps, nonfinite numbers, and stale partial workbook results; independent owned baseline data; unique active worksheet identification; atomic destination replacement and missing-only fallback. Root calculation flags remain; workbook fetching uses `rates fetch`.
