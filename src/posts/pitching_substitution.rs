use std::fmt::{Debug, Formatter};
use serde_json::Value;
use anyhow::{Result, Context};
use crate::{pitching_stats, last_name};

pub struct PitchingSubstitution {
    old: String,
    old_era: f64,
    new_id: i64,
    new: String,
    new_era: f64,
    abbreviation: String,
    innings_pitched: String,
    hits: usize,
    earned_runs: usize,
    walks: usize,
    strikeouts: usize,
    pitches: usize,
}

impl PitchingSubstitution {
    pub fn from_play(
        play: &Value,
        abbreviation: &str,
        previous_pitcher: Value,
    ) -> Result<Self> {
        let previous_pitcher_inning_stats = previous_pitcher["people"][0]["stats"][0]["splits"].as_array().and_then(|value| value.last()).map(|x| &x["stat"]).unwrap_or(&Value::Null);
        let old = previous_pitcher["people"][0]["fullName"]
            .as_str()
            .context("Could not find old pitcher's name")?
            .to_owned();
        let new_id = play["player"]["id"]
            .as_i64()
            .context("Could not find new pitcher's name")?;
        let new_pitcher = ureq::get(&format!("https://statsapi.mlb.com/api/v1/people/{new_id}?hydrate=stats(group=[pitching],type=[gameLog])")).call()?.into_json::<Value>()?;
        let new = new_pitcher["people"][0]["fullName"]
            .as_str()
            .context("Could not find new pitcher's name")?
            .to_owned();
        let (new_era, _) = pitching_stats(new_pitcher)?;
        let abbreviation = abbreviation.to_owned();
        let innings_pitched = previous_pitcher_inning_stats["inningsPitched"].as_str().unwrap_or("0.0").to_owned();
        let hits = previous_pitcher_inning_stats["hits"].as_i64().unwrap_or(0) as usize;
        let earned_runs = previous_pitcher_inning_stats["earnedRuns"].as_i64().unwrap_or(0) as usize;
        let strikeouts = previous_pitcher_inning_stats["strikeOuts"].as_i64().unwrap_or(0) as usize;
        let pitches = previous_pitcher_inning_stats["numberOfPitches"].as_i64().unwrap_or(0) as usize;
        let walks = (previous_pitcher_inning_stats["baseOnBalls"].as_i64().unwrap_or(0) + previous_pitcher_inning_stats["intentionalWalks"].as_i64().unwrap_or(0)) as usize;
        let (old_era, _) = pitching_stats(previous_pitcher)?;

        Ok(Self {
            old,
            old_era,
            new_id,
            new,
            new_era,
            abbreviation,
            innings_pitched,
            hits,
            earned_runs,
            walks,
            strikeouts,
            pitches,
        })
    }
    pub fn new_id(&self) -> i64 {
        self.new_id
    }

    pub fn last_name(&self) -> &str {
        last_name(&self.old)
    }
}

impl Debug for PitchingSubstitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            old,
            old_era,
            new,
            new_era,
            new_id: _,
            abbreviation,
            innings_pitched,
            hits,
            earned_runs,
            walks,
            strikeouts,
            pitches,
        } = self;
        writeln!(f, "### [{abbreviation} Pitching Change] | {new} ({new_era:.2} ERA) replaces {old} ({old_era:.2} ERA).")?;
        write!(f, "__{last_name}'s Final Line__:", last_name = self.last_name())?;
        writeln!(f, "\n> **{innings_pitched}** IP | **{hits}** H | **{earned_runs}** ER | **{walks}** BB | **{strikeouts}** K")?;
        writeln!(f, "> Pitch Count: **{pitches}**")?;
        writeln!(f, "")?;
        writeln!(f, "")?;

        Ok(())
    }
}