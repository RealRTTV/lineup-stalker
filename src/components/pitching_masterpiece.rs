use crate::posts::pitching_line::PitchingLine;
use mlb_api::game::TeamWithGameData;
use std::fmt::{Display, Formatter};
use fxhash::FxHashMap;
use mlb_api::person::{Ballplayer, PersonId};

#[derive(Clone)]
pub struct PitchingMasterpiece {
    team_abbreviation: String,
    line: PitchingLine,
    pitcher_names: Vec<String>,
    kind: &'static str,
}

impl PitchingMasterpiece {
    pub fn new(team: &TeamWithGameData, team_abbreviation: &str, all_players: &FxHashMap<PersonId, Ballplayer<()>>) -> Option<Self> {
        let runs = team.team_stats.pitching.runs.unwrap_or_default();
        let hits = team.team_stats.pitching.hits.unwrap_or_default();
        let base_on_balls = team.team_stats.pitching.base_on_balls.unwrap_or_default();
        let intentional_walks = team.team_stats.pitching.intentional_walks.unwrap_or_default();
        let errors = team.team_stats.fielding.errors.unwrap_or_default();
        let is_complete_game = team.pitchers.len() == 1;
        
        let masterpiece_kind = if hits == 0 {
            if base_on_balls + intentional_walks + errors == 0 {
                Some("Perfect Game")
            } else {
                Some("No-Hitter")
            }
        } else if is_complete_game {
            if runs == 0 {
                Some("Complete Game Shutout")
            } else {
                Some("Complete Game")
            }
        } else {
            None
        };
        
        Some(Self {
            team_abbreviation: team_abbreviation.to_owned(),
            line: PitchingLine::from_stats(&team.team_stats.pitching, is_complete_game, true),
            pitcher_names: team.pitchers.iter().map(|id| all_players[id].boxscore_name.clone()).collect(),
            kind: masterpiece_kind?,
        })
    }
}

impl Display for PitchingMasterpiece {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "### {abbreviation} {combined}{kind}{maddux_suffix}\n:star: __{pitcher_names}'s Final Line__ :star:\n{line}",
            abbreviation = self.team_abbreviation,
            kind = self.kind,
            combined = if !self.line.is_complete_game() { "Combined " } else { "" },
            maddux_suffix = if self.line.is_maddux() { " Maddux" } else { "" },
            pitcher_names = self.pitcher_names.join(", "),
            line = self.line,
        )
    }
}
