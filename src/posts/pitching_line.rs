use std::fmt::{Debug, Formatter};
use anyhow::Result;
use mlb_api::game::PlayerWithGameData;
use mlb_api::stats::{CountingStat, InningsPitched};

#[derive(Clone)]
pub struct PitcherFinalLine {
    boxscore_name: String,
    
    innings_pitched: InningsPitched,
    hits: CountingStat,
    earned_runs: CountingStat,
    walks: CountingStat,
    strikeouts: CountingStat,
    pitches: CountingStat,
}

impl PitcherFinalLine {
    pub fn from_play(pitcher: &PlayerWithGameData) -> Self {
        Self {
            boxscore_name: pitcher.boxscore_name.clone(),
            innings_pitched: pitcher.stats.pitching.innings_pitched.unwrap_or_default(),
            hits: pitcher.stats.pitching.hits.unwrap_or_default(),
            earned_runs: pitcher.stats.pitching.earned_runs.unwrap_or_default(),
            walks: pitcher.stats.pitching.base_on_balls.unwrap_or_default(),
            strikeouts: pitcher.stats.pitching.strikeouts.unwrap_or_default(),
            pitches: pitcher.stats.pitching.number_of_pitches.unwrap_or_default(),
        }
    }
}

impl Debug for PitcherFinalLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            boxscore_name,
            innings_pitched,
            hits,
            earned_runs,
            walks,
            strikeouts,
            pitches,
        } = self;
        writeln!(f, "### __{boxscore_name}'s Final Line__:")?;
        writeln!(f, "\n> **{innings_pitched}** IP | **{hits}** H | **{earned_runs}** ER | **{walks}** BB | **{strikeouts}** K")?;
        writeln!(f, "> Pitch Count: **{pitches}**")?;
        writeln!(f, "")?;

        Ok(())
    }
}