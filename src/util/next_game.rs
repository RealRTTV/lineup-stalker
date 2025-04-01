use std::fmt::{Display, Formatter};
use std::str::FromStr;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value;

use crate::get;

#[derive(Clone)]
pub struct NextGame {
    home: bool,
    location: String,
    utc: NaiveDateTime,
}

const SPECIFY_TEAM_LOCATIONS: &[&str] = &["New York", "Los Angeles", "Chicago"];

impl NextGame {
    pub fn new(game: &Value, our_id: i64) -> Result<Self> {
        let home_id = game["teams"]["home"]["team"]["id"].as_i64().context("Could not get home Team ID")?;
        let away_id = game["teams"]["away"]["team"]["id"].as_i64().context("Could not get away Team ID")?;
        let home = home_id == our_id;
        let opponent_team = get(&format!("https://statsapi.mlb.com/api/v1/teams/{}", if home { away_id } else { home_id }))?;
        let location_name = opponent_team["teams"][0]["franchiseName"].as_str().context("Could not get team franchise name")?.to_string();
        let full_name = opponent_team["teams"][0]["name"].as_str().context("Could not get team full name")?.to_string();
        Ok(Self {
            home,
            location: if SPECIFY_TEAM_LOCATIONS.contains(&location_name.as_str()) { full_name } else { location_name },
            utc: DateTime::<Utc>::from_str(game["gameDate"].as_str().context("Game Date Time didn't exist")?)?.naive_utc(),
        })
    }
}

impl Display for NextGame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{home} {location} (<t:{timestamp}:f>)",
               home = if self.home { "vs." } else { "@" },
               location = self.location,
               timestamp = self.utc.timestamp(),
        )
    }
}