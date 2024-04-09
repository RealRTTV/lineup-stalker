use std::fmt::{Display, Formatter};
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::Value;

use crate::{get, TIMEZONE};

pub struct NextGame {
    home: bool,
    location: String,
    utc: NaiveDateTime,
    local_timezone: Tz,
}

const SPECIFY_TEAM_LOCATIONS: &[&str] = &["New York", "Los Angeles", "Chicago"];

impl NextGame {
    pub fn new(game: &Value, our_id: i64) -> Result<Self> {
        let home_id = game["teams"]["home"]["team"]["id"].as_i64().context("Could not get home Team ID")?;
        let home_team = get(&format!("https://statsapi.mlb.com/api/v1/teams/{home_id}"))?;
        let location_name = home_team["teams"][0]["franchiseName"].as_str().context("Could not get team franchise name")?.to_string();
        let full_name = home_team["teams"][0]["name"].as_str().context("Could not get team full name")?.to_string();
        Ok(Self {
            home: home_id == our_id,
            location: if SPECIFY_TEAM_LOCATIONS.contains(&location_name.as_str()) { full_name } else { location_name },
            utc: DateTime::<Utc>::from_str(game["gameDate"].as_str().context("Game Date Time didn't exist")?)?.naive_utc(),
            local_timezone: Tz::from_str(game["venue"]["timeZone"]["id"].as_str().context("Could not find venue's local time zone for game")?).map_err(|e| anyhow!("{e}"))?,
        })
    }
}

impl Display for NextGame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let your_time = TIMEZONE.from_utc_datetime(&self.utc);
        let local_time = self.local_timezone.from_utc_datetime(&self.utc);
        write!(f, "{home} {location} ({date} @ {time})",
               home = if self.home { "vs." } else { "@" },
               location = self.location,
               date = self.utc.date().format("%m/%d"),
               time = if your_time.naive_local() != local_time.naive_local() { format!("{your} / {local}", your = your_time.format("%H:%M %Z"), local = local_time.format("%H:%M %Z")) } else { your_time.format("%H:%M %Z").to_string() }
        )
    }
}