use std::fmt::{Display, Formatter};
use mlb_api::Handedness;
use mlb_api::stats::{InningsPitched, TwoDecimalPlaceRateStat};

#[derive(Clone)]
pub struct PitcherLineupEntry {
    name: String,
    team_abbreviation: String,
    handedness: Handedness,
    era: TwoDecimalPlaceRateStat,
    innings_pitched: InningsPitched,
}

impl PitcherLineupEntry {
    pub fn new(name: String, team_abbreviation: String, handedness: Handedness, era: TwoDecimalPlaceRateStat, innings_pitched: InningsPitched) -> Self {
        Self {
            name,
            team_abbreviation,
            handedness,
            era,
            innings_pitched,
        }
    }
}

impl Display for PitcherLineupEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { name, team_abbreviation, handedness, era, innings_pitched } = self;
        write!(f, "`{handedness}` | **{team_abbreviation}** {name} ({era} ERA *|* {ip} IP)")
    }
}