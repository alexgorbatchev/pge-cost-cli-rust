use crate::{
    calculator::{calculate, normalize_plan},
    model::*,
    spreadsheet::*,
};
use anyhow::{Context, Result, ensure};
use cli_command_kit::{
    HelpOptions, Mode,
    clap::{Arg, ArgAction, ArgMatches, Command, value_parser},
    display_path,
};
use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL};
use std::{
    ffi::OsString,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

const SKILL: &str = "Install the pge-cost skill using the Skills CLI:\n\n  npx skills add alexgorbatchev/pge-cost-cli --skill pge-cost\n\nSkill:\n\n  https://github.com/alexgorbatchev/pge-cost-cli/tree/main/skills/pge-cost\n";

fn db_arg() -> Arg {
    Arg::new("db")
        .long("db")
        .value_name("PATH")
        .value_parser(value_parser!(PathBuf))
        .default_value("rates.json")
        .help("Rates JSON file")
}

fn estimate(command: Command) -> Command {
    command
        .arg(
            Arg::new("watts")
                .long("watts")
                .value_name("WATTS")
                .required(true)
                .value_parser(|s: &str| -> std::result::Result<f64, String> {
                    let watts = s
                        .parse::<f64>()
                        .map_err(|_| "watts must be a number".to_owned())?;
                    if watts.is_finite() && watts > 0.0 {
                        Ok(watts)
                    } else {
                        Err("watts must be finite and greater than zero".into())
                    }
                })
                .help("Continuous device wattage"),
        )
        .arg(
            Arg::new("plan")
                .long("plan")
                .value_name("PLAN")
                .required(true)
                .value_parser(|s: &str| {
                    normalize_plan(s)
                        .map(str::to_owned)
                        .map_err(|error| error.to_string())
                })
                .help("E-1, E-TOU-C, E-TOU-D, EV2, E-ELEC, or EV-B"),
        )
        .arg(
            Arg::new("tier")
                .long("tier")
                .value_name("TIER")
                .value_parser(value_parser!(i32))
                .default_value("2")
                .help("E-1 marginal tier, 1 or 2"),
        )
        .arg(
            Arg::new("year")
                .long("year")
                .value_name("YEAR")
                .value_parser(value_parser!(i32).range(1..=9999))
                .default_value("2026")
                .help("Calendar year for hourly weights"),
        )
        .arg(db_arg())
}

pub fn command() -> Command {
    estimate(
        Command::new("pge-cost")
            .about("Estimate costs for a device running continuously on PG&E residential rates"),
    )
    .subcommand_negates_reqs(true)
    .args_conflicts_with_subcommands(true)
    .arg(
        Arg::new("version")
            .long("version")
            .short('v')
            .action(ArgAction::SetTrue)
            .exclusive(true)
            .help("Print the raw version"),
    )
    .subcommand(
        Command::new("cost")
            .about("Estimate device electricity costs")
            .subcommand_required(true)
            .subcommand(estimate(
                Command::new("estimate").about("Calculate weighted continuous running costs"),
            )),
    )
    .subcommand(
        Command::new("rates")
            .about("Manage electricity rates")
            .subcommand_required(true)
            .subcommand(
                Command::new("fetch")
                    .about("Download the official workbook and replace local rates")
                    .arg(db_arg())
                    .arg(
                        Arg::new("url")
                            .long("url")
                            .value_name("URL")
                            .default_value(RATES_URL)
                            .value_parser(|s: &str| -> std::result::Result<reqwest::Url, String> {
                                let url = reqwest::Url::parse(s)
                                    .map_err(|_| "URL must be absolute HTTP or HTTPS".to_owned())?;
                                if matches!(url.scheme(), "http" | "https") {
                                    Ok(url)
                                } else {
                                    Err("URL must use HTTP or HTTPS".into())
                                }
                            })
                            .help("Residential rates XLSX source URL"),
                    ),
            ),
    )
}

pub fn help_options() -> HelpOptions {
    HelpOptions {
        skill: Some(SKILL.into()),
        ..Default::default()
    }
}

#[derive(Debug)]
struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}
impl std::error::Error for Cancelled {}

pub async fn fetch_rates(
    url: &reqwest::Url,
    path: &Path,
    cancel: CancellationToken,
) -> Result<RatesDatabase> {
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "rate source URL must not contain credentials"
    );
    let operation = async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(90))
            .retry(reqwest::retry::never())
            .build()
            .context("creating rates HTTP client")?;
        let response = client
            .get(url.clone())
            .send()
            .await
            .context("downloading rates workbook")?;
        ensure!(
            response.status() == reqwest::StatusCode::OK,
            "rates download returned HTTP {}",
            response.status()
        );
        let bytes = response.bytes().await.context("reading rates workbook")?;
        let base = serde_json::from_str(DEFAULT_RATES).context("reading bundled rates")?;
        let rates = parse_spreadsheet(&bytes, &base)?;
        if cancel.is_cancelled() {
            return Err(Cancelled.into());
        }
        save_database(path, &rates)?;
        Ok(rates)
    };
    tokio::select! { biased; _ = cancel.cancelled() => Err(Cancelled.into()), result = operation => result }
}

fn table(headers: &[&str]) -> Table {
    let mut table = Table::new();
    table
        .load_style(UTF8_FULL.with_rounded_corners())
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(cli_command_kit::terminal_width().clamp(20, usize::from(u16::MAX)) as u16)
        .set_header(headers);
    table
}

async fn perform(
    matches: &ArgMatches,
    options: &HelpOptions,
    output: &mut impl Write,
    mode: Mode,
    cancel: CancellationToken,
) -> Result<()> {
    if cli_command_kit::write_skill(matches, options, output)? {
        return Ok(());
    }
    if matches.get_flag("version") {
        writeln!(output, "{}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    if let Some(("rates", parent)) = matches.subcommand() {
        let fetch = parent
            .subcommand_matches("fetch")
            .expect("required fetch command");
        let url = fetch.get_one::<reqwest::Url>("url").unwrap();
        let path = fetch.get_one::<PathBuf>("db").unwrap();
        let rates = fetch_rates(url, path, cancel).await?;
        if mode == Mode::Agent {
            serde_json::to_writer(
                &mut *output,
                &serde_json::json!({"output": display_path(path), "last_updated": rates.last_updated, "plans": rates.plans}),
            )?;
            writeln!(output)?;
        } else {
            let mut summary = table(&["Plan", "Summer / Tier 1", "Winter / Tier 2"]);
            for (id, plan) in rates.plans {
                let (summer, winter) = if let Some(rate) = plan.rates {
                    (rate.summer.peak, rate.winter.peak)
                } else {
                    (plan.tier_1, plan.tier_2)
                };
                summary.add_row([id, format!("${summer:.5}"), format!("${winter:.5}")]);
            }
            writeln!(
                output,
                "Saved rates: {}\nUpdated: {}\n{summary}",
                display_path(path),
                rates.last_updated
            )?;
        }
        return Ok(());
    }
    let estimate = matches
        .subcommand_matches("cost")
        .and_then(|parent| parent.subcommand_matches("estimate"))
        .unwrap_or(matches);
    let plan_id = estimate.get_one::<String>("plan").unwrap();
    let tier = *estimate.get_one::<i32>("tier").unwrap();
    ensure!(
        plan_id != "E-1" || matches!(tier, 1 | 2),
        "E-1 tier must be 1 or 2"
    );
    let path = estimate.get_one::<PathBuf>("db").unwrap();
    let (database, embedded) = load_database(path)?;
    let plan = database
        .plans
        .get(plan_id)
        .context("selected plan is missing from rates database")?;
    let result = calculate(
        plan_id,
        plan,
        *estimate.get_one::<f64>("watts").unwrap(),
        tier,
        *estimate.get_one::<i32>("year").unwrap(),
    )?;
    let source = if embedded {
        "embedded".into()
    } else {
        display_path(path)
    };
    if mode == Mode::Agent {
        let mut value = serde_json::to_value(&result)?;
        value["database"] = source.clone().into();
        value["embedded"] = embedded.into();
        value["last_updated"] = database.last_updated.into();
        serde_json::to_writer(&mut *output, &value)?;
        writeln!(output)?;
    } else {
        let mut specs = table(&["Parameter", "Value"]);
        specs.add_row([
            "Device wattage".into(),
            format!("{:.1} W ({:.3} kW)", result.watts, result.watts / 1000.0),
        ]);
        let plan = if plan_id == "E-1" {
            format!("{} (Tier {tier})", result.plan_name)
        } else {
            result.plan_name.clone()
        };
        specs.add_row(["Selected plan".into(), format!("{plan_id} — {plan}")]);
        specs.add_row([
            "Effective rate".into(),
            format!("${:.5}/kWh (Weighted 24/7)", result.effective_rate),
        ]);
        let mut costs = table(&["Period", "Energy", "Cost"]);
        for (label, hours, cost) in [
            ("Daily (24 hours)".to_owned(), 24.0, result.daily_cost),
            (
                "Monthly (annual / 12)".to_owned(),
                f64::from(result.annual_hours) / 12.0,
                result.monthly_cost,
            ),
            (
                format!("Annual ({} hours)", result.annual_hours),
                f64::from(result.annual_hours),
                result.annual_cost,
            ),
        ] {
            costs.add_row([
                label,
                format!("{:.2} kWh", hours * result.watts / 1000.0),
                format!("${cost:.2}"),
            ]);
        }
        writeln!(
            output,
            "{specs}\n\n{costs}\n\nRates: {source} (updated {})\nCalendar: {} Pacific time\nMarginal estimates use total bundled energy rates.",
            database.last_updated, result.year
        )?;
    }
    Ok(())
}

pub async fn entry(
    args: impl IntoIterator<Item = OsString>,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    mode: Mode,
    cancel: CancellationToken,
) -> u8 {
    let options = help_options();
    let root =
        cli_command_kit::setup(command(), mode, &options).expect("valid static command tree");
    let result = match root.try_get_matches_from(args) {
        Ok(matches) => perform(&matches, &options, output, mode, cancel).await,
        Err(error) if !error.use_stderr() => output
            .write_all(error.to_string().as_bytes())
            .map_err(Into::into),
        Err(error) => {
            let _ = write!(diagnostics, "{error}");
            return 1;
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let prefix = if mode == Mode::Agent {
                "ERR:"
            } else {
                "[ERROR]"
            };
            let _ = writeln!(diagnostics, "{prefix} {error:#}");
            if error.is::<Cancelled>() { 130 } else { 1 }
        }
    }
}
