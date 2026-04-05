use mlb_api::game::Linescore;
use std::fmt::{Display, Formatter, Write};
use mlb_api::HomeAway;
use mlb_api::team::Team;

#[derive(Clone)]
pub struct LineScore {
    header: String,
    away_linescore: String,
    home_linescore: String,
}

impl LineScore {
    pub fn new(linescore: &Linescore, teams: HomeAway<&Team<()>>) -> Result<Self> {
        let mut header = "**`    ".to_owned();
        let mut away_linescore = format!("`{abbreviation: <3} ", abbreviation = teams.away.name.abbreviation);
        let mut home_linescore = format!("`{abbreviation: <3} ", abbreviation = teams.home.name.abbreviation);

        for inning in &linescore.innings {
            write!(
                &mut header,
                "|{n: ^3}",
                n = inning.inning,
            )?;
            write!(
                &mut away_linescore,
                "|{n: ^3}",
                n = inning.inning_record.away.runs,
            )?;
            write!(
                &mut home_linescore,
                "|{n: ^3}",
                n = if inning.inning_record.home.was_inning_half_played {
                    "-".to_owned()
                } else {
                    inning.inning_record.home.runs.to_string()
                }
            )?;
        }
        let runs_width = u32::max(linescore.rhe_totals.home.runs.checked_ilog10().map_or(1, |x| x + 1), linescore.rhe_totals.away.runs.checked_ilog10().map_or(1, |x| x + 1)) as usize;
        let hits_width = u32::max(linescore.rhe_totals.home.hits.checked_ilog10().map_or(1, |x| x + 1), linescore.rhe_totals.away.hits.checked_ilog10().map_or(1, |x| x + 1)) as usize;
        let errors_width = u32::max(linescore.rhe_totals.home.errors.checked_ilog10().map_or(1, |x| x + 1), linescore.rhe_totals.away.errors.checked_ilog10().map_or(1, |x| x + 1)) as usize;
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
            r = linescore.rhe_totals.away.runs,
            h = linescore.rhe_totals.away.hits,
            e = linescore.rhe_totals.away.errors
        )?;
        write!(
            &mut home_linescore,
            "|| {r: >runs_width$} | {h: >hits_width$} | {e: >errors_width$} |`",
            r = linescore.rhe_totals.home.runs,
            h = linescore.rhe_totals.home.hits,
            e = linescore.rhe_totals.home.errors
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
