use crate::components::hitting::HitterLineupEntry;
use crate::components::pitching::PitcherLineupEntry;
use crate::components::record_against::RecordAgainst;
use crate::components::standings::Standings;
use crate::posts::Post;
use crate::util::stat::HittingStat;
use crate::util::statsapi::Score;
use chrono::DateTime;
use chrono_tz::Tz;
use mlb_api::HomeAway;
use std::fmt::Display;

#[derive(Clone)]
pub struct Lineup {
    datetime: DateTime<Tz>,
    title: String,
    time: String,
    previous: Option<Score>,
    pub record: RecordAgainst,
    pub standings: Standings,
    probable_pitchers: HomeAway<PitcherLineupEntry>,
    hitting_stats: [HittingStat; 2],
    lineup: [HitterLineupEntry; 9],
}

impl Lineup {
    pub fn new(
        datetime: DateTime<Tz>,
        title: String,
        time: String,
        previous: Option<Score>,
        record: RecordAgainst,
        standings: Standings,
        pitchers: HomeAway<PitcherLineupEntry>,
        hitting_stats: [HittingStat; 2],
        lineup: [HitterLineupEntry; 9],
    ) -> Self {
        Self {
            datetime,
            title,
            time,
            previous,
            record,
            standings,
            probable_pitchers: pitchers,
            hitting_stats,
            lineup,
        }
    }
    
    pub fn update_probable_pitchers(&mut self, probable_pitchers: HomeAway<PitcherLineupEntry>) {
        self.probable_pitchers = probable_pitchers;
    }

    pub fn update_lineup(&mut self, lineup: [HitterLineupEntry; 9]) {
        self.lineup = lineup;
    }
}

impl Display for Lineup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { datetime, title, time, previous, record, standings, probable_pitchers: pitchers, hitting_stats: [first_stat, second_stat], lineup } = self;

        writeln!(f, "# {} {title}", datetime.format("%m*|*%d*|*%y"))?;
        writeln!(f, "First Pitch: {time}")?;
        if let Some(previous) = previous {
            writeln!(f, "Previous Game: {previous}")?;
        }
        writeln!(f, "Record Against: {record}")?;
        writeln!(f, "Standings: {standings}")?;
        writeln!(f, "### __Starting Pitchers__")?;
        writeln!(f, "{}", pitchers.away)?;
        writeln!(f, "{}", pitchers.home)?;
        writeln!(f, "### __Starting Lineup (.{first_stat} *|* .{second_stat})__")?;
        for line in lineup {
            writeln!(f, "{line}")?;
        }
        write!(f, "> ")?;

        Ok(())
    }
}

impl Post for Lineup {}
