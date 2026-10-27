use crate::model::*;
use anyhow::{Context, Result, bail, ensure};
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Timelike, Weekday};
use chrono_tz::America::Los_Angeles;
use std::collections::HashSet;

pub fn normalize_plan(value: &str) -> Result<&'static str> {
    Ok(match value.trim().to_ascii_uppercase().as_str() {
        "E1" | "E-1" => "E-1",
        "ETOUC" | "E-TOU-C" | "TOUC" | "TOU-C" => "E-TOU-C",
        "ETOUD" | "E-TOU-D" | "TOUD" | "TOU-D" => "E-TOU-D",
        "EV2" | "EV2A" | "EV2-A" | "EV-2A" => "EV2",
        "EELEC" | "E-ELEC" | "ELEC" => "E-ELEC",
        "EVB" | "EV-B" => "EV-B",
        _ => bail!("unknown rate plan; choose E-1, E-TOU-C, E-TOU-D, EV2, E-ELEC, or EV-B"),
    })
}

pub fn calculate(
    plan_id: &str,
    plan: &RatePlan,
    watts: f64,
    tier: i32,
    year: i32,
) -> Result<CalculationResult> {
    ensure!(
        watts.is_finite() && watts > 0.0,
        "watts must be finite and greater than zero"
    );
    ensure!(
        (1..=9999).contains(&year),
        "year must be between 1 and 9999"
    );
    let plan_id = normalize_plan(plan_id)?;
    let (effective_rate, annual_hours) = if plan_id == "E-1" {
        let rate = match tier {
            1 => plan.tier_1,
            2 => plan.tier_2,
            _ => bail!("E-1 tier must be 1 or 2"),
        };
        ensure!(
            rate.is_finite() && rate > 0.0,
            "E-1 tier {tier} rate must be finite and positive"
        );
        let days = NaiveDate::from_ymd_opt(year + 1, 1, 1)
            .unwrap()
            .signed_duration_since(NaiveDate::from_ymd_opt(year, 1, 1).unwrap())
            .num_days();
        (rate, (days * 24) as u32)
    } else {
        let hours = tou_hours(
            year,
            plan.seasons.as_ref().context("plan seasons are missing")?,
            plan.schedule.as_ref().context("plan schedule is missing")?,
            plan_id == "EV-B",
        )?;
        let rates = plan.rates.as_ref().context("plan rates are missing")?;
        let mut numerator = 0.0;
        let mut total = 0;
        for (counts, rates) in hours.iter().zip([&rates.summer, &rates.winter]) {
            for (&count, rate) in
                counts
                    .iter()
                    .zip([rates.peak, rates.partial_peak, rates.off_peak])
            {
                ensure!(
                    rate.is_finite() && (count == 0 || rate > 0.0),
                    "used TOU rates must be finite and positive"
                );
                numerator += f64::from(count) * rate;
                total += count;
            }
        }
        (numerator / f64::from(total), total)
    };
    let daily_energy_kwh = watts * 24.0 / 1000.0;
    let daily_cost = daily_energy_kwh * effective_rate;
    let annual_cost = watts / 1000.0 * f64::from(annual_hours) * effective_rate;
    ensure!(
        [effective_rate, daily_energy_kwh, daily_cost, annual_cost]
            .iter()
            .all(|v| v.is_finite()),
        "calculation exceeds finite numeric range"
    );
    Ok(CalculationResult {
        plan_id: plan_id.into(),
        plan_name: plan.name.clone(),
        watts,
        effective_rate,
        daily_energy_kwh,
        daily_cost,
        monthly_cost: annual_cost / 12.0,
        annual_cost,
        annual_hours,
        year,
    })
}

pub fn in_season(date: NaiveDate, season: &SeasonConfig) -> bool {
    let current = (date.month(), date.day());
    let start = (season.start_month, season.start_day);
    let end = (season.end_month, season.end_day);
    if start <= end {
        start <= current && current <= end
    } else {
        current >= start || current <= end
    }
}

fn schedule_periods(schedule: &DaySchedule) -> Result<[usize; 24]> {
    let mut periods = [None; 24];
    for (period, ranges) in [&schedule.peak, &schedule.partial_peak, &schedule.off_peak]
        .iter()
        .enumerate()
    {
        for range in *ranges {
            ensure!(
                range.start < range.end && range.end <= 24,
                "time ranges must satisfy 0 <= start < end <= 24"
            );
            for slot in &mut periods[range.start as usize..range.end as usize] {
                ensure!(slot.is_none(), "day schedule has overlapping time ranges");
                *slot = Some(period);
            }
        }
    }
    ensure!(
        periods.iter().all(Option::is_some),
        "day schedule must cover all 24 hours"
    );
    Ok(periods.map(Option::unwrap))
}

pub fn tou_hours(
    year: i32,
    seasons: &SeasonConfigs,
    schedule: &PlanSchedule,
    ev_b: bool,
) -> Result<[[u32; 3]; 2]> {
    ensure!(
        (1..=9999).contains(&year),
        "year must be between 1 and 9999"
    );
    for season in [&seasons.summer, &seasons.winter] {
        ensure!(
            NaiveDate::from_ymd_opt(2000, season.start_month, season.start_day).is_some()
                && NaiveDate::from_ymd_opt(2000, season.end_month, season.end_day).is_some(),
            "season contains an invalid month or day"
        );
    }
    let (everyday, weekdays, weekends) = if let Some(everyday) = &schedule.everyday {
        ensure!(
            schedule.weekdays.is_none() && schedule.weekends_holidays.is_none(),
            "use either everyday or weekday/weekend schedules"
        );
        (Some(schedule_periods(everyday)?), None, None)
    } else {
        (
            None,
            Some(schedule_periods(
                schedule
                    .weekdays
                    .as_ref()
                    .context("weekday schedule is missing")?,
            )?),
            Some(schedule_periods(
                schedule
                    .weekends_holidays
                    .as_ref()
                    .context("weekend/holiday schedule is missing")?,
            )?),
        )
    };
    let holidays = holidays(year);
    let mut instant = Los_Angeles
        .with_ymd_and_hms(year, 1, 1, 0, 0, 0)
        .single()
        .context("ambiguous start of year")?
        .to_utc();
    let end = Los_Angeles
        .with_ymd_and_hms(year + 1, 1, 1, 0, 0, 0)
        .single()
        .context("ambiguous end of year")?
        .to_utc();
    let mut hours = [[0; 3]; 2];
    ensure!(
        (end - instant).num_seconds() % 3600 == 0,
        "calendar year has a historical Pacific offset change shorter than one hour"
    );
    while instant < end {
        let local = instant.with_timezone(&Los_Angeles);
        let date = local.date_naive();
        let summer = in_season(date, &seasons.summer);
        let winter = in_season(date, &seasons.winter);
        ensure!(summer != winter, "seasons must cover {date} exactly once");
        let weekend =
            matches!(local.weekday(), Weekday::Sat | Weekday::Sun) || holidays.contains(&date);
        let periods = everyday.as_ref().unwrap_or_else(|| {
            if weekend {
                weekends.as_ref().unwrap()
            } else {
                weekdays.as_ref().unwrap()
            }
        });
        let hour = if ev_b && ev_b_shift(date) {
            (local.hour() + 23) % 24
        } else {
            local.hour()
        };
        hours[usize::from(!summer)][periods[hour as usize]] += 1;
        instant += Duration::hours(1);
    }
    Ok(hours)
}

// Schedule EV's published transition weeks use periods one hour later.
pub fn ev_b_shift(date: NaiveDate) -> bool {
    let year = date.year();
    (date >= nth_weekday(year, 3, Weekday::Sun, 2) && date < nth_weekday(year, 4, Weekday::Sun, 1))
        || (date >= last_weekday(year, 10, Weekday::Sun)
            && date < nth_weekday(year, 11, Weekday::Sun, 1))
}

pub fn holidays(year: i32) -> HashSet<NaiveDate> {
    let mut result = HashSet::new();
    for holiday_year in year - 1..=year + 1 {
        for (month, day) in [(1, 1), (7, 4), (11, 11), (12, 25)] {
            let date = NaiveDate::from_ymd_opt(holiday_year, month, day).unwrap();
            let observed = match date.weekday() {
                Weekday::Sat => date - Duration::days(1),
                Weekday::Sun => date + Duration::days(1),
                _ => date,
            };
            if observed.year() == year {
                result.insert(observed);
            }
        }
    }
    result.extend([
        nth_weekday(year, 2, Weekday::Mon, 3),
        last_weekday(year, 5, Weekday::Mon),
        nth_weekday(year, 9, Weekday::Mon, 1),
        nth_weekday(year, 11, Weekday::Thu, 4),
    ]);
    result
}

fn nth_weekday(year: i32, month: u32, weekday: Weekday, nth: u32) -> NaiveDate {
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    first
        + Duration::days(i64::from(
            (weekday.num_days_from_monday() + 7 - first.weekday().num_days_from_monday()) % 7
                + (nth - 1) * 7,
        ))
}

fn last_weekday(year: i32, month: u32, weekday: Weekday) -> NaiveDate {
    let last = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap() - Duration::days(1);
    last - Duration::days(i64::from(
        (last.weekday().num_days_from_monday() + 7 - weekday.num_days_from_monday()) % 7,
    ))
}
