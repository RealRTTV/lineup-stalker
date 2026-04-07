use mlb_api::person::PersonId;
use mlb_api::stats::{InningsPitched, TwoDecimalPlaceRateStat};
use mlb_api::Handedness;
use std::fmt::{Display, Formatter};
use crate::util::hide;

#[derive(Clone)]
pub struct PitcherLineupEntry {
    name: String,
    team_abbreviation: String,
    handedness: Option<Handedness>,
    era: TwoDecimalPlaceRateStat,
    innings_pitched: Option<InningsPitched>,
    id: PersonId,
}

impl PitcherLineupEntry {
    pub fn new(name: String, id: PersonId, team_abbreviation: String, handedness: Handedness, era: TwoDecimalPlaceRateStat, innings_pitched: InningsPitched) -> Self {
        Self {
            name,
            team_abbreviation,
            handedness: Some(handedness),
            era,
            innings_pitched: Some(innings_pitched),
            id,
        }
    }

    pub fn unknown(name: String) -> Self {
        Self {
            name,
            team_abbreviation: hide("___"),
            handedness: None,
            era: TwoDecimalPlaceRateStat::NIL,
            innings_pitched: None,
            id: PersonId::new(0),
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
        let handedness = (*handedness).map_or('-', Handedness::into_char);
        let innings_pitched = (*innings_pitched).map_or_else(|| "--.-".to_owned(), |ip| ip.to_string());
        write!(f, "`{handedness}` | **{team_abbreviation}** {name} ({era} ERA *|* {innings_pitched} IP)")
    }
}