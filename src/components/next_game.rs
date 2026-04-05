use std::fmt::{Display, Formatter};
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use mlb_api::request::RequestURLBuilderExt;
use mlb_api::schedule::ScheduleGame;
use mlb_api::team::{Team, TeamId, TeamsRequest};
use mlb_api::{HomeAway, TeamSide};

#[derive(Clone)]
pub struct NextGame {
    cheering_for: TeamSide,
    location: String,
    utc: NaiveDateTime,
}

impl NextGame {
    pub async fn new(game: &ScheduleGame<()>, our_id: TeamId) -> Result<Self> {
        let cheering_for = if game.teams.home.team.id == our_id { TeamSide::Home } else { TeamSide::Away };
        let [opponent_team]: [Team<()>; 1] = TeamsRequest::builder().team_id(game.teams.as_ref().choose(!cheering_for).team.id).build_and_get().await?.teams.try_into().context("Expected exactly one team in response")?;
        Ok(Self {
            cheering_for,
            location: opponent_team.name.short_name,
            utc: DateTime::<Utc>::from_str(game["gameDate"].as_str().context("Game Date Time didn't exist")?)?.naive_utc(),
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