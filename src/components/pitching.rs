use mlb_api::stats::{InningsPitched, TwoDecimalPlaceRateStat};
use mlb_api::Handedness;
use std::fmt::{Display, Formatter};
use mlb_api::person::PersonId;

#[derive(Clone)]
pub struct PitcherLineupEntry {
    name: String,
    team_abbreviation: String,
    handedness: Handedness,
    era: TwoDecimalPlaceRateStat,
    innings_pitched: InningsPitched,
    id: PersonId,
}

impl PitcherLineupEntry {
    pub fn new(name: String, id: PersonId, team_abbreviation: String, handedness: Handedness, era: TwoDecimalPlaceRateStat, innings_pitched: InningsPitched) -> Self {
        Self {
            name,
            team_abbreviation,
            handedness,
            era,
            innings_pitched,
            id,
        }
    }
    
    #[must_use]
    pub fn id(&self) -> PersonId {
        self.id
    }
}

impl Display for PitcherLineupEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { name, team_abbreviation, handedness, era, innings_pitched, id: _ } = self;
        write!(f, "`{handedness}` | **{team_abbreviation}** {name} ({era} ERA *|* {innings_pitched} IP)")
    }
}