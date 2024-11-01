use std::fmt::{Display, Formatter};

#[derive(Clone)]
pub struct HitterLineupEntry {
    batting_position: u8,
    fielding_position: String,
    name: String,
    stats: Option<(String, String)>,
}

impl HitterLineupEntry {
    pub fn new(name: String, fielding_position: String, batting_position: u8, stats: Option<(String, String)>) -> Self {
        Self {
            batting_position,
            fielding_position,
            name,
            stats,
        }
    }
}

impl Display for HitterLineupEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { batting_position, fielding_position, name, stats } = self;
        write!(f, r"`{batting_position}` | **{fielding_position}** {name}{stats_value}", stats_value = if let Some((first_stat_value, second_stat_value)) = stats { format!(" [{first_stat_value} *|* {second_stat_value}]") } else { String::new() })
    }
}
