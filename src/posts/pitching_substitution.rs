use std::fmt::{Debug, Formatter};
use serde_json::Value;
use anyhow::{Result, Context};
use crate::{pitching_stats, get};

#[derive(Clone)]
pub struct PitchingSubstitution {
    old_name: String,
    old_last_name: String,
    old_era: f64,
    new_id: i64,
    new_name: String,
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
        let old_name = previous_pitcher["people"][0]["fullName"]
            .as_str()
            .context("Could not find old pitcher's name")?
            .to_owned();
        let old_last_name = previous_pitcher["people"][0]["lastName"]
            .as_str()
            .context("Could not find old pitcher's last name")?
            .to_owned();
        let new_id = play["player"]["id"]
            .as_i64()
            .context("Could not find new pitcher's name")?;
        let new_pitcher = get(&format!("https://statsapi.mlb.com/api/v1/people/{new_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?;
        let new_name = new_pitcher["people"][0]["fullName"]
            .as_str()
            .context("Could not find new pitcher's name")?
            .to_owned();
        let (new_era, _, _) = pitching_stats(new_pitcher)?;
        let abbreviation = abbreviation.to_owned();
        let innings_pitched = previous_pitcher_inning_stats["inningsPitched"].as_str().unwrap_or("0.0").to_owned();
        let hits = previous_pitcher_inning_stats["hits"].as_i64().unwrap_or(0) as usize;
        let earned_runs = previous_pitcher_inning_stats["earnedRuns"].as_i64().unwrap_or(0) as usize;
        let strikeouts = previous_pitcher_inning_stats["strikeOuts"].as_i64().unwrap_or(0) as usize;
        let pitches = previous_pitcher_inning_stats["numberOfPitches"].as_i64().unwrap_or(0) as usize;
        let walks = (previous_pitcher_inning_stats["baseOnBalls"].as_i64().unwrap_or(0) + previous_pitcher_inning_stats["intentionalWalks"].as_i64().unwrap_or(0)) as usize;
        let (old_era, _, _) = pitching_stats(previous_pitcher)?;

        Ok(Self {
            old_name,
            old_last_name,
            old_era,
            new_id,
            new_name,
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

    pub fn old_last_name(&self) -> &str {
        &self.old_last_name
    }
}

impl Debug for PitchingSubstitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            old_name,
            old_last_name,
            old_era,
            new_name,
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
        writeln!(f, "### [{abbreviation} Pitching Change] | {new_name} ({new_era:.2} ERA) replaces {old_name} ({old_era:.2} ERA).")?;
        write!(f, "__{old_last_name}'s Final Line__:")?;
        writeln!(f, "\n> **{innings_pitched}** IP | **{hits}** H | **{earned_runs}** ER | **{walks}** BB | **{strikeouts}** K")?;
        writeln!(f, "> Pitch Count: **{pitches}**")?;
        writeln!(f, "")?;

        Ok(())
    }
}