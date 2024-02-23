use std::fmt::{Display, Formatter};
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::Value;

use crate::TIMEZONE;

pub struct NextGame {
    home: bool,
    opponent: String,
    utc: NaiveDateTime,
    local_timezone: Tz,
}

impl NextGame {
    pub fn new(game: &Value, our_id: i64) -> Result<Self> {

        Ok(Self {
            home: game["teams"]["home"]["team"]["id"].as_i64().is_some_and(|id| id == our_id),
            opponent: game["venue"]["location"]["city"].as_str().context("Expected location 'city' to exist")?.to_owned(),
            utc: DateTime::<Utc>::from_str(game["gameDate"].as_str().context("Game Date Time didn't exist")?)?.naive_utc(),
            local_timezone: Tz::from_str(game["venue"]["timeZone"]["id"].as_str().context("Could not find venue's local time zone for game")?).map_err(|e| anyhow!("{e}"))?,
        })
    }
}

impl Display for NextGame {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{location} {opponent} ({date} @ {your_time} / {local_time})",
               location = if self.home { "vs." } else { "@" },
               opponent = self.opponent,
               date = self.utc.date().format("%m/%d"),
               your_time = TIMEZONE.from_utc_datetime(&self.utc).format("%H:%M %Z"),
               local_time = self.local_timezone.from_utc_datetime(&self.utc).format("%H:%M %Z")
        )
    }
}