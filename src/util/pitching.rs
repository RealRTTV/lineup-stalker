use std::fmt::{Display, Formatter};

#[derive(Clone)]
pub struct PitcherLineupEntry {
    name: String,
    team_abbreviation: String,
    handedness: char,
    era: f64,
    ip: f64,
}

impl PitcherLineupEntry {
    pub fn new(name: String, team_abbreviation: String, handedness: char, era: f64, ip: f64) -> Self {
        Self {
            name,
            team_abbreviation,
            handedness,
            era,
            ip,
        }
    }
}

impl Display for PitcherLineupEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { name, team_abbreviation, handedness, era, ip } = self;
        write!(f, "`{handedness}` | **{team_abbreviation}** {name} ({era:.2} ERA *|* {ip:.1} IP)")
    }
}