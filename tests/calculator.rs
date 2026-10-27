use chrono::{Datelike, NaiveDate};
use pge_cost_cli::{calculator::*, model::*};

fn db() -> RatesDatabase {
    serde_json::from_str(DEFAULT_RATES).unwrap()
}
fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
}

#[test]
fn preserves_six_plans_and_prices_with_real_calendar_weights() {
    let db = db();
    for (plan, expected) in [
        ("E-1", 0.40702),
        ("E-TOU-C", 0.390935),
        ("E-TOU-D", 0.35279),
        ("EV2", 0.30303),
        ("E-ELEC", 0.32593),
        ("EV-B", 0.345141),
    ] {
        let result = calculate(plan, &db.plans[plan], 150.0, 2, 2026).unwrap();
        assert!(
            (result.effective_rate - expected).abs() < 0.00002,
            "{plan}: {result:?}"
        );
        assert!((result.daily_energy_kwh - 3.6).abs() < 1e-12);
        assert_eq!(result.annual_hours, 8760);
        assert!((result.annual_cost - 1314.0 * result.effective_rate).abs() < 1e-9);
        assert!((result.monthly_cost * 12.0 - result.annual_cost).abs() < 1e-9);
    }
    assert_eq!(
        calculate("E-1", &db.plans["E-1"], 100.0, 1, 2026)
            .unwrap()
            .effective_rate,
        0.32561
    );
    for year in [2024, 2028, 2000] {
        let r = calculate("E-1", &db.plans["E-1"], 1000.0, 2, year).unwrap();
        assert_eq!(r.annual_hours, 8784);
        assert!((r.annual_cost - 8784.0 * 0.40702).abs() < 1e-9);
    }
    let p = &db.plans["E-TOU-C"];
    let hours = tou_hours(
        2026,
        p.seasons.as_ref().unwrap(),
        p.schedule.as_ref().unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(hours, [[610, 0, 2318], [1215, 0, 4617]]);
    let mut custom = p.clone();
    custom
        .schedule
        .as_mut()
        .unwrap()
        .everyday
        .as_mut()
        .unwrap()
        .peak = vec![TimeRange { start: 1, end: 2 }];
    custom
        .schedule
        .as_mut()
        .unwrap()
        .everyday
        .as_mut()
        .unwrap()
        .off_peak = vec![
        TimeRange { start: 0, end: 1 },
        TimeRange { start: 2, end: 24 },
    ];
    let h = tou_hours(
        2026,
        custom.seasons.as_ref().unwrap(),
        custom.schedule.as_ref().unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(h[0][0], 122);
    assert_eq!(h[1][0], 244); // the repeated 01:00 hour in Pacific fall-back time
}

#[test]
fn season_boundaries_and_observed_holidays_are_exact() {
    let summer = SeasonConfig {
        start_month: 6,
        start_day: 10,
        end_month: 6,
        end_day: 20,
    };
    for (day, expected) in [(9, false), (10, true), (15, true), (20, true), (21, false)] {
        assert_eq!(in_season(date(2026, 6, day), &summer), expected);
    }
    let winter = db().plans["EV-B"].seasons.as_ref().unwrap().winter.clone();
    assert!(in_season(date(2026, 12, 31), &winter));
    assert!(in_season(date(2026, 1, 1), &winter));
    assert!(!in_season(date(2026, 5, 1), &winter));
    let h = holidays(2026);
    for d in [
        (1, 1),
        (2, 16),
        (5, 25),
        (7, 3),
        (9, 7),
        (11, 11),
        (11, 26),
        (12, 25),
    ] {
        assert!(h.contains(&date(2026, d.0, d.1)));
    }
    assert!(!h.contains(&date(2026, 7, 4)));
    assert!(holidays(2021).contains(&date(2021, 12, 31))); // next New Year's observed in this year
    assert!(holidays(2023).contains(&date(2023, 1, 2)));
    assert!(h.iter().all(|d| d.year() == 2026));
    for (month, day, expected) in [
        (3, 7, false),
        (3, 8, true),
        (4, 4, true),
        (4, 5, false),
        (10, 24, false),
        (10, 25, true),
        (10, 31, true),
        (11, 1, false),
    ] {
        assert_eq!(ev_b_shift(date(2026, month, day)), expected);
    }
    for (alias, expected) in [
        (" E1 ", "E-1"),
        ("ETOUC", "E-TOU-C"),
        ("TOU-C", "E-TOU-C"),
        ("ETOUD", "E-TOU-D"),
        ("TOUD", "E-TOU-D"),
        ("TOU-D", "E-TOU-D"),
        ("EV2A", "EV2"),
        ("EV2-A", "EV2"),
        ("EV-2A", "EV2"),
        ("EELEC", "E-ELEC"),
        ("ELEC", "E-ELEC"),
        ("EVB", "EV-B"),
    ] {
        assert_eq!(normalize_plan(alias).unwrap(), expected);
    }
}

#[test]
fn invalid_numbers_incomplete_schedules_and_bad_calendar_data_are_errors() {
    let db = db();
    let p = &db.plans["E-TOU-C"];
    for watts in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MAX] {
        assert!(calculate("E-TOU-C", p, watts, 2, 2026).is_err());
    }
    for year in [0, -1, 10000] {
        assert!(calculate("E-TOU-C", p, 1.0, 2, year).is_err());
    }
    assert!(calculate("E-TOU-C", p, 1.0, 2, 1883).is_err()); // Pacific local mean time changed by a fraction of an hour
    assert!(calculate("UNKNOWN", p, 100.0, 2, 2026).is_err());
    assert!(calculate("E-1", &db.plans["E-1"], 100.0, 3, 2026).is_err());
    for tier in [1, 2] {
        let mut bad = db.plans["E-1"].clone();
        bad.tier_1 = 0.0;
        bad.tier_2 = f64::INFINITY;
        assert!(calculate("E-1", &bad, 100.0, tier, 2026).is_err());
    }
    for field in 0..9 {
        let mut bad = p.clone();
        match field {
            0 => bad.seasons = None,
            1 => bad.schedule = None,
            2 => bad.rates = None,
            3 => bad.rates.as_mut().unwrap().summer.peak = -1.0,
            4 => bad.seasons.as_mut().unwrap().summer.start_day = 40,
            5 => bad.seasons.as_mut().unwrap().summer.end_month = 11,
            6 => {
                bad.schedule
                    .as_mut()
                    .unwrap()
                    .everyday
                    .as_mut()
                    .unwrap()
                    .peak[0]
                    .end = 25
            }
            7 => bad
                .schedule
                .as_mut()
                .unwrap()
                .everyday
                .as_mut()
                .unwrap()
                .off_peak
                .clear(),
            _ => {
                bad.schedule
                    .as_mut()
                    .unwrap()
                    .everyday
                    .as_mut()
                    .unwrap()
                    .peak[0]
                    .start = 10
            }
        }
        assert!(
            calculate("E-TOU-C", &bad, 100.0, 2, 2026).is_err(),
            "field {field}"
        );
    }
    let mut bad = db.plans["E-TOU-D"].clone();
    bad.schedule.as_mut().unwrap().weekdays = None;
    assert!(calculate("E-TOU-D", &bad, 100.0, 2, 2026).is_err());
    bad = db.plans["E-TOU-D"].clone();
    bad.schedule.as_mut().unwrap().weekends_holidays = None;
    assert!(calculate("E-TOU-D", &bad, 100.0, 2, 2026).is_err());
}
