use crate::util::statsapi::stalker_abbreviation;
use anyhow::Result;
use mlb_api::game::Linescore;
use mlb_api::team::Team;
use mlb_api::HomeAway;
use std::fmt::{Display, Formatter, Write};

#[derive(Clone)]
pub struct LineScore {
    header: String,
    away_linescore: String,
    home_linescore: String,
}

impl LineScore {
    pub fn new(linescore: &Linescore, teams: HomeAway<&Team<()>>) -> Result<Self> {
        let mut header = "**`    ".to_owned();
        let mut away_linescore = format!("`{abbreviation: <3} ", abbreviation = stalker_abbreviation(&teams.away.name));
        let mut home_linescore = format!("`{abbreviation: <3} ", abbreviation = stalker_abbreviation(&teams.home.name));

        for inning in &linescore.innings {
            write!(
                &mut header,
                "|{n: ^3}",
                n = *inning.inning,
            )?;
            write!(
                &mut away_linescore,
                "|{n: ^3}",
                n = if inning.inning_record.away.was_inning_half_played {
                    inning.inning_record.away.runs.to_string()
                } else {
                    "-".to_owned()
                },
            )?;
            write!(
                &mut home_linescore,
                "|{n: ^3}",
                n = if inning.inning_record.home.was_inning_half_played {
                    inning.inning_record.home.runs.to_string()
                } else {
                    "-".to_owned()
                }
            )?;
        }
        write!(
            &mut header,
            "||{r: ^3}|{h: ^3}|{e: ^3}|`**",
            r = "R",
            h = "H",
            e = "E",
        )?;
        write!(
            &mut away_linescore,
            "||{r: ^3}|{h: ^3}|{e: ^3}|`",
            r = linescore.rhe_totals.away.runs,
            h = linescore.rhe_totals.away.hits,
            e = linescore.rhe_totals.away.errors
        )?;
        write!(
            &mut home_linescore,
            "||{r: ^3}|{h: ^3}|{e: ^3}|`",
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
