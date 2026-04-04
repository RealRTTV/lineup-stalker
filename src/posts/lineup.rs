use std::fmt::Display;
use chrono::DateTime;
use chrono_tz::Tz;
use crate::posts::components::hitting::HitterLineupEntry;
use crate::posts::components::pitching::PitcherLineupEntry;
use crate::posts::components::record_against::RecordAgainst;
use crate::posts::components::standings::Standings;
use crate::util::stat::HittingStat;
use crate::util::statsapi::Score;

#[derive(Clone)]
pub struct Lineup {
    datetime: DateTime<Tz>,
    title: String,
    time: String,
    previous: Option<Score>,
    pub record: RecordAgainst,
    pub standings: Standings,
    home_pitcher_stats: PitcherLineupEntry,
    away_pitcher_stats: PitcherLineupEntry,
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
        home_pitcher_stats: PitcherLineupEntry,
        away_pitcher_stats: PitcherLineupEntry,
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
            home_pitcher_stats,
            away_pitcher_stats,
            hitting_stats,
            lineup,
        }
    }

    pub fn update_lineup(&mut self, lineup: [HitterLineupEntry; 9]) {
        self.lineup = lineup;
    }
}

impl Display for Lineup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { datetime, title, time, previous, record, standings, home_pitcher_stats, away_pitcher_stats, hitting_stats: [first_stat, second_stat], lineup } = self;

        writeln!(f, "# {} {title}", datetime.format("%m*|*%d*|*%y"))?;
        writeln!(f, "First Pitch: {time}")?;
        if let Some(previous) = previous {
            writeln!(f, "Previous Game: {previous:?}")?;
        }
        writeln!(f, "Record Against: {record}")?;
        writeln!(f, "Standings: {standings}")?;
        writeln!(f, "### __Starting Pitchers__")?;
        writeln!(f, "{away_pitcher_stats}")?;
        writeln!(f, "{home_pitcher_stats}")?;
        writeln!(f, "### __Starting Lineup (.{first_stat_value} *|* .{second_stat_value})__", first_stat_value = first_stat.to_string(), second_stat_value = second_stat.to_string())?;
        for line in lineup {
            writeln!(f, "{line}")?;
        }
        write!(f, "> ")?;

        Ok(())
    }
}
