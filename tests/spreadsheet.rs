use pge_cost_cli::{model::*, spreadsheet::*};
use rust_xlsxwriter::Workbook;

fn workbook(mode: u8) -> Vec<u8> {
    let mut book = Workbook::new();
    let main = book.add_worksheet();
    main.set_name("Res Inclu TOU_300101-Present").unwrap();
    main.write_string(2, 0, "E1, ESR, ES,  ET").unwrap();
    main.write_string(2, 8, "$0.11").unwrap();
    main.write_number(2, 9, 0.22).unwrap();
    for (row, label) in [(5, "Rate Schedule E-TOU-C"), (10, "Rate Schedule E-TOU-D")] {
        main.write_string(row, 0, label).unwrap();
        for offset in 0..4 {
            if !(mode == 1 && row == 10 && offset == 3) {
                main.write_number(row + offset, 9, 0.3 + f64::from(offset) / 100.0)
                    .unwrap();
            }
        }
    }
    if mode == 2 {
        main.write_string(15, 0, "Rate Schedule E-TOU-C").unwrap();
    }
    if mode == 3 {
        main.write_number(3, 8, 0.15).unwrap();
        main.write_number(3, 9, 0.25).unwrap();
    }
    if mode == 4 {
        main.write_number(5, 9, -0.5).unwrap();
    }
    if mode == 5 {
        main.write_string(5, 9, "NaN").unwrap();
    }
    let tech = book.add_worksheet();
    tech.set_name("ElecVehicle&Tech_300101-Present").unwrap();
    for (row, label) in [
        (3, "Rate Schedule EV, Rate B"),
        (11, "Rate Schedule EV2"),
        (19, "Rate Schedule E-ELEC"),
    ] {
        tech.write_string(row, 0, label).unwrap();
        for offset in 0..6 {
            tech.write_number(row + offset, 8, 0.4 + f64::from(offset) / 100.0)
                .unwrap();
        }
    }
    if mode == 6 {
        book.add_worksheet()
            .set_name("Res Inclu TOU_310101-Present")
            .unwrap();
    }
    if mode == 7 {
        book.add_worksheet()
            .set_name("ElecVehicle&Tech_310101-Present")
            .unwrap();
    }
    if mode == 8 {
        book.worksheet_from_index(0)
            .unwrap()
            .write_string(5, 9, "$$0.3")
            .unwrap();
    }
    book.save_to_buffer().unwrap()
}

#[test]
fn native_xlsx_updates_all_plans_without_mutating_baseline() {
    let base: RatesDatabase = serde_json::from_str(DEFAULT_RATES).unwrap();
    let before = serde_json::to_value(&base).unwrap();
    let rates = parse_spreadsheet(&workbook(0), &base).unwrap();
    assert_eq!(serde_json::to_value(&base).unwrap(), before);
    assert_eq!(rates.plans["E-1"].tier_1, 0.11);
    assert_eq!(rates.plans["E-1"].tier_2, 0.22);
    for name in ["E-TOU-C", "E-TOU-D"] {
        let rate = rates.plans[name].rates.as_ref().unwrap();
        assert_eq!(rate.summer.peak, 0.3);
        assert_eq!(rate.summer.off_peak, 0.31);
        assert!((rate.winter.off_peak - 0.33).abs() < 1e-12);
    }
    for name in ["EV-B", "EV2", "E-ELEC"] {
        let rate = rates.plans[name].rates.as_ref().unwrap();
        assert_eq!(rate.summer.peak, 0.4);
        assert!((rate.winter.off_peak - 0.45).abs() < 1e-12);
    }
    assert!(chrono::NaiveDate::parse_from_str(&rates.last_updated, "%Y-%m-%d").is_ok());
}

#[test]
fn rejects_incomplete_ambiguous_or_invalid_rates_instead_of_stale_values() {
    let base: RatesDatabase = serde_json::from_str(DEFAULT_RATES).unwrap();
    for mode in 1..=8 {
        assert!(
            parse_spreadsheet(&workbook(mode), &base).is_err(),
            "mode {mode}"
        );
    }
    assert!(parse_spreadsheet(b"not a workbook", &base).is_err());
    let mut empty = Workbook::new();
    empty.add_worksheet();
    assert!(parse_spreadsheet(&empty.save_to_buffer().unwrap(), &base).is_err());
    let mut incomplete = base.clone();
    incomplete.plans.remove("EV2");
    assert!(parse_spreadsheet(&workbook(0), &incomplete).is_err());
}

#[test]
fn database_fallback_is_only_for_missing_files_and_writes_are_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rates.json");
    let (base, fallback) = load_database(&path).unwrap();
    assert!(fallback);
    save_database(&path, &base).unwrap();
    assert!(!load_database(&path).unwrap().1);
    std::fs::write(&path, b"invalid JSON").unwrap();
    assert!(load_database(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"invalid JSON");
    assert!(load_database(dir.path()).is_err());
    assert!(save_database(dir.path(), &base).is_err());
    let nested = dir.path().join("nested/rates.json");
    save_database(&nested, &base).unwrap();
    assert_eq!(load_database(&nested).unwrap().0.plans.len(), 6);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    let before = std::fs::read(&nested).unwrap();
    let mut invalid = base.clone();
    invalid.plans.get_mut("E-1").unwrap().tier_1 = f64::NAN;
    assert!(save_database(&nested, &invalid).is_err());
    assert_eq!(std::fs::read(&nested).unwrap(), before);
}

#[tokio::test]
async fn real_http_fetch_is_atomic_on_bad_status_bad_workbook_and_cancellation() {
    use cli_command_kit::Mode;
    use std::{ffi::OsString, time::Duration};
    use tokio_util::sync::CancellationToken;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rates.json");
    for mode in [Mode::Human, Mode::Agent] {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/rates.xlsx", server.server_addr());
        let bytes = workbook(0);
        let task = std::thread::spawn(move || {
            let request = server.recv().unwrap();
            assert_eq!(request.method().as_str(), "GET");
            assert_eq!(request.url(), "/rates.xlsx");
            request
                .respond(tiny_http::Response::from_data(bytes))
                .unwrap();
        });
        let args = [
            OsString::from("pge-cost"),
            "rates".into(),
            "fetch".into(),
            "--url".into(),
            url.into(),
            "--db".into(),
            path.as_os_str().to_owned(),
        ];
        let mut output = Vec::new();
        let mut error = Vec::new();
        assert_eq!(
            pge_cost_cli::entry(
                args,
                &mut output,
                &mut error,
                mode,
                CancellationToken::new()
            )
            .await,
            0,
            "{}",
            String::from_utf8_lossy(&error)
        );
        task.join().unwrap();
        assert_eq!(load_database(&path).unwrap().0.plans["E-1"].tier_1, 0.11);
        if mode == Mode::Agent {
            assert!(
                serde_json::from_slice::<serde_json::Value>(&output).unwrap()["plans"].is_object()
            );
        } else {
            assert!(String::from_utf8(output).unwrap().contains("Saved rates:"));
        }
    }
    let original = std::fs::read(&path).unwrap();
    for status in [200, 503] {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = reqwest::Url::parse(&format!("http://{}", server.server_addr())).unwrap();
        let task = std::thread::spawn(move || {
            server
                .recv()
                .unwrap()
                .respond(
                    tiny_http::Response::from_string("invalid workbook").with_status_code(status),
                )
                .unwrap();
        });
        assert!(
            pge_cost_cli::fetch_rates(&url, &path, CancellationToken::new())
                .await
                .is_err()
        );
        task.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = reqwest::Url::parse(&format!("http://{}", server.server_addr())).unwrap();
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let task = std::thread::spawn(move || {
        let _request = server.recv().unwrap();
        signal.cancel();
        std::thread::sleep(Duration::from_millis(50));
    });
    let mut output = Vec::new();
    let mut error = Vec::new();
    let args = [
        OsString::from("pge-cost"),
        "rates".into(),
        "fetch".into(),
        "--url".into(),
        url.to_string().into(),
        "--db".into(),
        path.as_os_str().to_owned(),
    ];
    let status = tokio::time::timeout(
        Duration::from_secs(3),
        pge_cost_cli::entry(args, &mut output, &mut error, Mode::Agent, cancel),
    )
    .await
    .unwrap();
    task.join().unwrap();
    assert_eq!(status, 130);
    assert!(output.is_empty());
    assert_eq!(error, b"ERR: cancelled\n");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(
        pge_cost_cli::fetch_rates(&url, &path, cancel)
            .await
            .is_err()
    );
}
