use std::fmt::{Display, Formatter};
use mlb_api::game::BattingOrderIndex;
use mlb_api::meta::NamedPosition;

#[derive(Clone, Debug)]
pub struct HitterLineupEntry {
    batting_order: BattingOrderIndex,
    position: NamedPosition,
    name: String,
    stats: Option<[String; 2]>,
}

impl HitterLineupEntry {
    pub fn new(name: String, position: NamedPosition, batting_order: BattingOrderIndex, stats: Option<[String; 2]>) -> Self {
        Self {
            batting_order,
            position,
            name,
            stats,
        }
    }
}

impl Display for HitterLineupEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { batting_order, position, name, stats } = self;
        let position = &position.abbreviation;
        let stats = stats.map(|stats| format!(" [{}]", stats.join(" *|* "))).unwrap_or_default();
        write!(f, r"`{batting_order}` | **{position}** {name}{stats}")
    }
}
