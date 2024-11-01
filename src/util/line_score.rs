use std::fmt::{Display, Formatter, Write};
use anyhow::Context;
use serde_json::Value;
use crate::util::team_stats_log::TeamStatsLog;

#[derive(Clone)]
pub struct LineScore {
    header: String,
    away_linescore: String,
    home_linescore: String,
}

impl LineScore {
    pub fn new(innings: &[Value], away: &TeamStatsLog, home: &TeamStatsLog, top: bool) -> anyhow::Result<Self> {
        let mut header = "**`    ".to_owned();
        let mut away_linescore = format!("`{abbreviation: <3} ", abbreviation = away.abbreviation);
        let mut home_linescore = format!("`{abbreviation: <3} ", abbreviation = home.abbreviation);

        for (idx, inning) in innings.iter().enumerate() {
            write!(
                &mut header,
                "|{n: ^3}",
                n = inning["num"]
                    .as_i64()
                    .context("Could not find inning number")?
            )?;
            write!(
                &mut away_linescore,
                "|{n: ^3}",
                n = inning["away"]["runs"].as_i64().unwrap_or(0)
            )?;
            write!(
                &mut home_linescore,
                "|{n: ^3}",
                n = if idx + 1 == innings.len() && top {
                    "-".to_owned()
                } else {
                    inning["home"]["runs"].as_i64().unwrap_or(0).to_string()
                }
            )?;
        }
        let runs_width = u32::max(home.runs.checked_ilog10().map_or(1, |x| x + 1), away.runs.checked_ilog10().map_or(1, |x| x + 1)) as usize;
        let hits_width = u32::max(home.hits.checked_ilog10().map_or(1, |x| x + 1), away.hits.checked_ilog10().map_or(1, |x| x + 1)) as usize;
        let errors_width = u32::max(home.errors.checked_ilog10().map_or(1, |x| x + 1), away.errors.checked_ilog10().map_or(1, |x| x + 1)) as usize;
        write!(
            &mut header,
            "|| {r: >runs_width$} | {h: >hits_width$} | {e: >errors_width$} |`**",
            r = "R",
            h = "H",
            e = "E",
        )?;
        write!(
            &mut away_linescore,
            "|| {r: >runs_width$} | {h: >hits_width$} | {e: >errors_width$} |`",
            r = away.runs,
            h = away.hits,
            e = away.errors
        )?;
        write!(
            &mut home_linescore,
            "|| {r: >runs_width$} | {h: >hits_width$} | {e: >errors_width$} |`",
            r = home.runs,
            h = home.hits,
            e = home.errors
        )?;
        Ok(Self {
            header,
            away_linescore,
            home_linescore,
        })
    }
}

impl Display for LineScore {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { header, away_linescore, home_linescore } = self;
        writeln!(f, "{header}")?;
        writeln!(f, "{away_linescore}")?;
        write!(f, "{home_linescore}")?;

        Ok(())
    }
}
