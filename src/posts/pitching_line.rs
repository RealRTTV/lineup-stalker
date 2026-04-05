use mlb_api::game::PlayerWithGameData;
use mlb_api::stats::{CountingStat, InningsPitched};
use std::fmt::{Debug, Display, Formatter};
use mlb_api::stats::raw::pitching;
use mlb_api::stats::wrappers::WithNone;
use crate::posts::Post;

#[derive(Clone)]
pub struct PitcherFinalLine {
    boxscore_name: String,
    pitching_line: PitchingLine,
}

impl PitcherFinalLine {
    pub fn from_play(pitcher: &PlayerWithGameData) -> Self {
        Self {
            boxscore_name: pitcher.boxscore_name.clone(),
            pitching_line: PitchingLine::from_stats(&pitcher.stats.pitching, false, false),
        }
    }
}

impl Display for PitcherFinalLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            boxscore_name,
            pitching_line
        } = self;
        writeln!(f, "### __{boxscore_name}'s Final Line__:")?;
        writeln!(f, "{pitching_line:?}")?;
        writeln!(f, "")?;
        Ok(())
    }
}

pub struct PitchingLine {
    innings_pitched: InningsPitched,
    hits: CountingStat,
    earned_runs: CountingStat,
    walks: CountingStat,
    strikeouts: CountingStat,
    pitches: CountingStat,
    
    runs: CountingStat,
    is_complete_game: bool,
    show_game_score: bool,
}

impl PitchingLine {
    pub fn from_stats(stats: &WithNone<pitching::__BoxscoreStatsData>, is_complete_game: bool, show_game_score: bool) -> Self {
        Self {
            innings_pitched: stats.innings_pitched.unwrap_or_default(),
            hits: stats.hits.unwrap_or_default(),
            earned_runs: stats.earned_runs.unwrap_or_default(),
            walks: stats.base_on_balls.unwrap_or_default(),
            strikeouts: stats.strikeouts.unwrap_or_default(),
            pitches: stats.number_of_pitches.unwrap_or_default(),
            
            runs: stats.runs.unwrap_or_default(),
            is_complete_game,
            show_game_score,
        }
    }
    
    pub fn game_score(&self) -> i32 {
        50 + self.innings_pitched.as_outs() as i32 + (2 * (self.innings_pitched.as_outs() - InningsPitched::new(4, 0).as_outs()) as i32) / 3 + self.strikeouts as i32 - 2 * self.hits as i32 - 4 * self.earned_runs as i32 - self.walks as i32
    }
    
    pub fn is_maddux(&self) -> bool {
        self.is_complete_game && self.runs == 0 && self.pitches < 100
    }
    
    pub fn is_complete_game(&self) -> bool {
        self.is_complete_game
    }
}

impl Display for PitchingLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { innings_pitched, hits, earned_runs, walks, strikeouts, pitches, .. } = self;
        let strikeout_surroundings = if *strikeouts >= 12 { "__" } else { "" };
        let is_maddux = self.is_maddux();
        let maddux_left = if is_maddux { "__*" } else { "" };
        let maddux_right = if is_maddux { "*__" } else { "" };
        write!(f, "**{innings_pitched}** IP, **{hits}** H, **{earned_runs}** ER, **{walks}** BB, **{strikeout_surroundings}{strikeouts}** K{strikeout_surroundings}, **{maddux_left}{pitches}{maddux_right}** P")?;
        if self.is_masterpiece() {
            write!(f, ", **{game_score}** GS", game_score = self.game_score())?;
        }
        Ok(())
    }
}

impl Debug for PitchingLine {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { innings_pitched, hits, earned_runs, walks, strikeouts, pitches, .. } = self;
        let strikeout_surroundings = if *strikeouts >= 12 { "__" } else { "" };
        let is_maddux = self.is_maddux();
        let maddux_left = if is_maddux { "__*" } else { "" };
        let maddux_right = if is_maddux { "*__" } else { "" };
        writeln!(f, "\n> **{innings_pitched}** IP | **{hits}** H | **{earned_runs}** ER | **{walks}** BB | **{strikeout_surroundings}{strikeouts}{strikeout_surroundings}** K")?;
        if self.is_masterpiece() {
            write!(f, ", **{game_score}** GS", game_score = self.game_score())?;
        }
        writeln!(f, "> Pitch Count: **{maddux_left}{pitches}{maddux_right}**")?;
        Ok(())
    }
}

impl Post for PitcherFinalLine {}
