use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_RATES: &str = include_str!("../rates.json");

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RatesDatabase {
    pub last_updated: String,
    pub plans: BTreeMap<String, RatePlan>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RatePlan {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tier_1: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tier_2: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seasons: Option<SeasonConfigs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<PlanSchedule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rates: Option<PlanRates>,
}

fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SeasonConfigs {
    pub summer: SeasonConfig,
    pub winter: SeasonConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SeasonConfig {
    pub start_month: u32,
    pub start_day: u32,
    pub end_month: u32,
    pub end_day: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlanSchedule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub everyday: Option<DaySchedule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekdays: Option<DaySchedule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekends_holidays: Option<DaySchedule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DaySchedule {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub peak: Vec<TimeRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partial_peak: Vec<TimeRange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub off_peak: Vec<TimeRange>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TimeRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlanRates {
    pub summer: SeasonRates,
    pub winter: SeasonRates,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SeasonRates {
    pub peak: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub partial_peak: f64,
    pub off_peak: f64,
}

#[derive(Debug, Serialize)]
pub struct CalculationResult {
    pub plan_id: String,
    pub plan_name: String,
    pub watts: f64,
    pub effective_rate: f64,
    pub daily_energy_kwh: f64,
    pub daily_cost: f64,
    pub monthly_cost: f64,
    pub annual_cost: f64,
    pub annual_hours: u32,
    pub year: i32,
}
