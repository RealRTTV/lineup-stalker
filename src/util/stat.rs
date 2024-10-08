#![allow(non_snake_case)]

use std::fmt::Display;
use serde_json::Value;
use anyhow::{Result, Context};
use crate::util;

#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum HittingStat {
    AVG,
    SLG,
    OBP,
    OPS,
    BABIP,
    BB,
    K,
    ISO,
    BBK,
    BPA,
    wOBA,
    wRCp,
}

impl HittingStat {
    pub const MAX_NAME_WIDTH: usize = 5;

    pub fn prev(self) -> Self {
        match self {
            Self::AVG => Self::wRCp,
            Self::SLG => Self::AVG,
            Self::OBP => Self::SLG,
            Self::OPS => Self::OBP,
            Self::BABIP => Self::OPS,
            Self::BB => Self::BABIP,
            Self::K => Self::BB,
            Self::ISO => Self::K,
            Self::BBK => Self::ISO,
            Self::BPA => Self::BBK,
            Self::wOBA => Self::BPA,
            Self::wRCp => Self::wOBA,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::AVG => Self::SLG,
            Self::SLG => Self::OBP,
            Self::OBP => Self::OPS,
            Self::OPS => Self::BABIP,
            Self::BABIP => Self::BB,
            Self::BB => Self::K,
            Self::K => Self::ISO,
            Self::ISO => Self::BBK,
            Self::BBK => Self::BPA,
            Self::BPA => Self::wOBA,
            Self::wOBA => Self::wRCp,
            Self::wRCp => Self::AVG,
        }
    }

    pub fn get(self, stats: &Value, team: &str) -> Result<String> {
        match self {
            Self::AVG => stats["avg"].as_str().map(str::to_owned).context("Could not get hitter's AVG"),
            Self::SLG => stats["slg"].as_str().map(str::to_owned).context("Could not get hitter's SLG"),
            Self::OBP => stats["obp"].as_str().map(str::to_owned).context("Could not get hitter's OBP"),
            Self::OPS => stats["ops"].as_str().map(str::to_owned).context("Could not get hitter's OPS"),
            Self::BABIP => stats["babip"].as_str().map(str::to_owned).context("Could not get hitter's BABIP"),
            Self::BB => {
                let bb = stats["baseOnBalls"].as_i64().context("Could not get player's BB count")? + stats["intentionalWalks"].as_i64().context("Could not get player's IBB count")?;
                let hbp = stats["hitByPitch"].as_i64().context("Could not get player's HBP count")?;
                let pa = stats["plateAppearances"].as_i64().context("Could not get player's PA count")?;
                let bb = (bb + hbp) as f64 / pa as f64;
                Ok(format!("{bb:.3}").split_off((bb < 1.0) as usize))
            }
            Self::K => {
                let k = stats["strikeOuts"].as_i64().context("Could not get player's K count")?;
                let pa = stats["plateAppearances"].as_i64().context("Could not get player's PA count")?;
                let k = k as f64 / pa as f64;
                Ok(format!("{k:.3}").split_off((k < 1.0) as usize))
            }
            Self::ISO => {
                let doubles = stats["doubles"].as_i64().context("Could not get player's doubles")?;
                let triples = stats["triples"].as_i64().context("Could not get player's triples")?;
                let home_runs = stats["homeRuns"].as_i64().context("Could not get player's home runs")?;
                let at_bats = stats["atBats"].as_i64().context("Could not get player's at bats count")?;

                let iso = (doubles + triples * 2 + home_runs * 3) as f64 / at_bats as f64;
                Ok(format!("{iso:.3}").split_off((iso < 1.0) as usize))
            }
            Self::BBK => {
                let bb = stats["baseOnBalls"].as_i64().context("Could not get player's BB count")? + stats["intentionalWalks"].as_i64().context("Could not get player's IBB count")?;
                let k = stats["strikeOuts"].as_i64().context("Could not get player's strikeouts")?;

                Ok(format!("{:.2}", bb as f64 / k as f64))
            }
            Self::BPA => {
                let doubles = stats["doubles"].as_i64().context("Could not get player's doubles")?;
                let triples = stats["triples"].as_i64().context("Could not get player's triples")?;
                let home_runs = stats["homeRuns"].as_i64().context("Could not get player's home runs")?;
                let singles = stats["hits"].as_i64().context("Could not get player's hits")? - doubles - triples - home_runs;
                let at_bats = stats["atBats"].as_i64().context("Could not get player's at bats count")?;
                let bb = stats["baseOnBalls"].as_i64().context("Could not get player's BB count")? + stats["intentionalWalks"].as_i64().context("Could not get player's IBB count")?;
                let hbp = stats["hitByPitch"].as_i64().context("Could not get player's HBP count")?;
                let sac = stats["sacFlies"].as_i64().context("Could not get player's sac flies")? + stats["sacBunts"].as_i64().context("Could not get player's sac bunts")?;
                let stolen_bases = stats["stolenBases"].as_i64().context("Could not get player's stolen bases")?;
                let caught_stealing = stats["caughtStealing"].as_i64().context("Could not get player's caught stealing")?;
                let grounded_into_double_plays = stats["groundIntoDoublePlay"].as_i64().context("Could not get player's GIDP")?;
                let bpa = (singles + stolen_bases + bb + hbp + doubles * 2 + triples * 3 + home_runs * 4 - caught_stealing - grounded_into_double_plays) as f64 / (at_bats + bb + hbp + sac) as f64;

                Ok(format!("{bpa:.3}").split_off((bpa < 1.0) as usize))
            }
            Self::wOBA => {
                let doubles = stats["doubles"].as_i64().context("Could not get player's doubles")? as usize;
                let triples = stats["triples"].as_i64().context("Could not get player's triples")? as usize;
                let home_runs = stats["homeRuns"].as_i64().context("Could not get player's home runs")? as usize;
                let singles = stats["hits"].as_i64().context("Could not get player's hits")? as usize - doubles - triples - home_runs;
                let at_bats = stats["atBats"].as_i64().context("Could not get player's at bats count")? as usize;
                let bb = stats["baseOnBalls"].as_i64().context("Could not get player's BB count")? as usize;
                let hbp = stats["hitByPitch"].as_i64().context("Could not get player's HBP count")? as usize;
                let sac = stats["sacFlies"].as_i64().context("Could not get player's sac flies")? as usize + stats["sacBunts"].as_i64().context("Could not get player's sac bunts")? as usize;
                let stolen_bases = stats["stolenBases"].as_i64().context("Could not get player's stolen bases")? as usize;
                let caught_stealings = stats["caughtStealing"].as_i64().context("Could not get player's caught stealing")? as usize;

                let wOBA = util::fangraphs::WOBA_CONSTANTS.calculate_wOBA(bb, hbp, singles, doubles, triples, home_runs, stolen_bases, caught_stealings, at_bats + bb + hbp + sac);
                Ok(format!("{wOBA:.3}").split_off((wOBA < 1.0) as usize))
            },
            Self::wRCp => {
                let doubles = stats["doubles"].as_i64().context("Could not get player's doubles")? as usize;
                let triples = stats["triples"].as_i64().context("Could not get player's triples")? as usize;
                let home_runs = stats["homeRuns"].as_i64().context("Could not get player's home runs")? as usize;
                let singles = stats["hits"].as_i64().context("Could not get player's hits")? as usize - doubles - triples - home_runs;
                let at_bats = stats["atBats"].as_i64().context("Could not get player's at bats count")? as usize;
                let bb = stats["baseOnBalls"].as_i64().context("Could not get player's BB count")? as usize;
                let ibb = stats["intentionalWalks"].as_i64().context("Could not get player's IBB count")? as usize;
                let hbp = stats["hitByPitch"].as_i64().context("Could not get player's HBP count")? as usize;
                let sac = stats["sacFlies"].as_i64().context("Could not get player's sac flies")? as usize + stats["sacBunts"].as_i64().context("Could not get player's sac bunts")? as usize;
                let stolen_bases = stats["stolenBases"].as_i64().context("Could not get player's stolen bases")? as usize;
                let caught_stealings = stats["caughtStealing"].as_i64().context("Could not get player's caught stealing")? as usize;

                let wRCp = util::fangraphs::WOBA_CONSTANTS.calculate_wRCp(bb, hbp, singles, doubles, triples, home_runs, stolen_bases, caught_stealings, at_bats + bb + hbp + sac + ibb, ibb, team);
                Ok(format!("{wRCp}"))
            }
        }
    }
}

impl Display for HittingStat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::AVG => "AVG",
            Self::SLG => "SLG",
            Self::OBP => "OBP",
            Self::OPS => "OPS",
            Self::BABIP => "BABIP",
            Self::BB => "BB",
            Self::K => "K",
            Self::ISO => "ISO",
            Self::BBK => "BB/K",
            Self::BPA => "BPA",
            Self::wOBA => "wOBA",
            Self::wRCp => "wRC+",
        })
    }
}
