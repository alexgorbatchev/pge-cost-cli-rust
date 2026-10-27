use crate::{calculator::calculate, model::*};
use anyhow::{Context, Result, ensure};
use calamine::{Data, Range, Reader, Xlsx};
use cli_command_kit::display_path;
use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
};

pub const RATES_URL: &str = "https://www.pge.com/assets/rates/tariffs/res-inclu-tou-current.xlsx";

pub fn load_database(path: &Path) -> Result<(RatesDatabase, bool)> {
    let (bytes, fallback) = match fs::read(path) {
        Ok(bytes) => (bytes, false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (DEFAULT_RATES.as_bytes().to_vec(), true)
        }
        Err(error) => {
            return Err(error).with_context(|| format!("reading rates {}", display_path(path)));
        }
    };
    let database = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing rates {}", display_path(path)))?;
    Ok((database, fallback))
}

pub fn save_database(path: &Path, database: &RatesDatabase) -> Result<()> {
    validate_database(database)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("creating {}", display_path(parent)))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("staging rates in {}", display_path(parent)))?;
    serde_json::to_writer_pretty(&mut file, database).context("encoding rates")?;
    file.write_all(b"\n").context("writing rates")?;
    file.as_file().sync_all().context("syncing rates")?;
    file.persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("saving rates {}", display_path(path)))?;
    Ok(())
}

pub fn parse_spreadsheet(bytes: &[u8], base: &RatesDatabase) -> Result<RatesDatabase> {
    let mut workbook = Xlsx::new(Cursor::new(bytes)).context("opening rates workbook")?;
    let names = workbook.sheet_names();
    let main = unique_sheet(&names, "Res Inclu TOU_")?;
    let tech = unique_sheet(&names, "ElecVehicle&Tech_")?;
    let main = workbook
        .worksheet_range(main)
        .context("reading residential worksheet")?;
    let tech = workbook
        .worksheet_range(tech)
        .context("reading vehicle worksheet")?;
    let mut result = base.clone();
    let row = marker(&main, "E1, ESR, ES,  ET")?;
    let mut tier_pairs = Vec::new();
    for target in row.saturating_sub(1)..=row + 1 {
        if let (Ok(first), Ok(second)) = (rate(&main, target, 8), rate(&main, target, 9))
            && !tier_pairs.contains(&(first, second))
        {
            tier_pairs.push((first, second));
        }
    }
    ensure!(
        tier_pairs.len() == 1,
        "E-1 needs one unambiguous positive tier-rate pair near its schedule row"
    );
    let e1 = result
        .plans
        .get_mut("E-1")
        .context("baseline E-1 plan is missing")?;
    (e1.tier_1, e1.tier_2) = tier_pairs[0];
    for (id, label, sheet, column, partial) in [
        ("E-TOU-C", "Rate Schedule E-TOU-C", &main, 9, false),
        ("E-TOU-D", "Rate Schedule E-TOU-D", &main, 9, false),
        ("EV-B", "Rate Schedule EV, Rate B", &tech, 8, true),
        ("EV2", "Rate Schedule EV2", &tech, 8, true),
        ("E-ELEC", "Rate Schedule E-ELEC", &tech, 8, true),
    ] {
        let row = marker(sheet, label)?;
        let season = |offset: u32| -> Result<SeasonRates> {
            Ok(SeasonRates {
                peak: rate(sheet, row + offset, column)
                    .with_context(|| format!("{id} peak rate"))?,
                partial_peak: if partial {
                    rate(sheet, row + offset + 1, column)
                        .with_context(|| format!("{id} partial-peak rate"))?
                } else {
                    0.0
                },
                off_peak: rate(sheet, row + offset + if partial { 2 } else { 1 }, column)
                    .with_context(|| format!("{id} off-peak rate"))?,
            })
        };
        result
            .plans
            .get_mut(id)
            .with_context(|| format!("baseline {id} plan is missing"))?
            .rates = Some(PlanRates {
            summer: season(0)?,
            winter: season(if partial { 3 } else { 2 })?,
        });
    }
    validate_database(&result)?;
    result.last_updated = chrono::Utc::now().format("%Y-%m-%d").to_string();
    Ok(result)
}

fn validate_database(database: &RatesDatabase) -> Result<()> {
    for (id, plan) in &database.plans {
        calculate(id, plan, 1.0, 2, 2026).with_context(|| format!("validating {id}"))?;
        if id == "E-1" {
            calculate(id, plan, 1.0, 1, 2026).context("validating E-1 tier 1")?;
        }
    }
    Ok(())
}

fn unique_sheet<'a>(names: &'a [String], prefix: &str) -> Result<&'a str> {
    let matching: Vec<_> = names
        .iter()
        .filter(|name| name.starts_with(prefix) && name.ends_with("-Present"))
        .collect();
    ensure!(
        matching.len() == 1,
        "workbook needs exactly one active {prefix} worksheet"
    );
    Ok(matching[0])
}

fn marker(sheet: &Range<Data>, label: &str) -> Result<u32> {
    let start = sheet.start().context("worksheet is empty")?.0;
    let end = sheet.end().unwrap().0;
    let matching: Vec<_> = (start..=end)
        .filter(|&row| {
            sheet
                .get_value((row, 0))
                .is_some_and(|cell| cell.to_string().contains(label))
        })
        .collect();
    ensure!(
        matching.len() == 1,
        "worksheet needs exactly one {label} schedule row"
    );
    Ok(matching[0])
}

fn rate(sheet: &Range<Data>, row: u32, column: u32) -> Result<f64> {
    let value = sheet
        .get_value((row, column))
        .context("rate cell is missing")?
        .to_string();
    let value = value.trim();
    let rate: f64 = value
        .strip_prefix('$')
        .unwrap_or(value)
        .trim()
        .parse()
        .context("rate cell is not a number")?;
    ensure!(
        rate.is_finite() && rate > 0.0,
        "rate cell must be finite and positive"
    );
    Ok(rate)
}
