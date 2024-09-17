use std::fmt::{Debug, Formatter};
use anyhow::{Result, Context};
use fxhash::FxHashMap;
use serde_json::Value;
use crate::util::nth;

pub struct DefensiveSubstitution {
    old: String,
    new: String,
    fielding_position: String,
    ordinal: u8,

    abbreviation: String,
    top: bool,
    inning: u8,
}

impl DefensiveSubstitution {
    pub fn from_play(
        play: &Value,
        parent: &Value,
        abbreviation: &str,
        id_to_object: &FxHashMap<i64, Value>,
    ) -> Result<Self> {
        Ok(Self {
            old: id_to_object.get(&play["replacedPlayer"]["id"].as_i64().context("Could not find old player in defensive substitution")?).context("Old Player ID wasn't in the roaster for either team")?["person"]["fullName"].as_str().context("Could not find old player's name in defensive substitution")?.to_owned(),
            new: id_to_object.get(&play["player"]["id"].as_i64().context("Could not find new player in defensive substitution")?).context("New Player ID wasn't in the roaster for either team")?["person"]["fullName"].as_str().context("Could not find new player's name in defensive substitution")?.to_owned(),
            fielding_position: play["position"]["abbreviation"].as_str().context("Could not find player's position in defensive substitution")?.to_owned(),
            ordinal: (play["battingOrder"].as_str().and_then(|s| s.parse::<usize>().ok()).context("Could not get defensive substitution's batting order")? / 100) as u8,

            abbreviation: abbreviation.to_owned(),
            top: parent["about"]["isTopInning"].as_bool().context("Could not find out if defensive substitution was in the top or bottom of the inning")?,
            inning: parent["about"]["inning"].as_i64().context("Could not find out defensive substitution inning")? as u8,
        })
    }
}

impl Debug for DefensiveSubstitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            old,
            new,
            fielding_position,
            ordinal,
            abbreviation,
            top,
            inning,
        } = self;

        writeln!(f, "### [{abbreviation} Lineup Change] | {new} replaces {old}, playing {fielding_position}, batting {n}.", n = nth(*ordinal as usize))?;
        writeln!(
            f,
            "> Inning: **{half} {n}**",
            half = if *top { "Top" } else { "Bot" },
            n = nth(*inning as usize)
        )?;
        writeln!(f, "")?;

        Ok(())
    }
}