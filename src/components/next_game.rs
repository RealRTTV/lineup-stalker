use std::fmt::{Display, Formatter};

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use mlb_api::request::RequestURLBuilderExt;
use mlb_api::schedule::ScheduleGame;
use mlb_api::team::{Team, TeamId, TeamsRequest};
use mlb_api::{HomeAway, TeamSide};

#[derive(Clone)]
pub struct NextGame {
    cheering_for: TeamSide,
    location: String,
    utc: DateTime<Utc>,
}

impl NextGame {
    pub async fn new(game: ScheduleGame<()>, our_id: TeamId) -> Result<Self> {
        let cheering_for = if game.teams.home.team.id == our_id { TeamSide::Home } else { TeamSide::Away };
        let Ok([opponent_team]): Result<[Team<()>; 1], _> = TeamsRequest::builder().team_id(game.teams.as_ref().choose(!cheering_for).team.id).build_and_get().await?.teams.try_into() else { bail!("Expected exactly one team in response") };
        Ok(Self {
            cheering_for,
            location: opponent_team.name.short_name,
            utc: game.game_date,
        })
    }
}

impl Display for NextGame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{home} {location} (<t:{timestamp}:f>)",
               home = HomeAway::new("vs.", "@").choose(self.cheering_for),
               location = self.location,
               timestamp = self.utc.timestamp(),
        )
    }
}