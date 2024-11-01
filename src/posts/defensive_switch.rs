use std::fmt::{Debug, Formatter};
use anyhow::{Result, Context};
use fxhash::FxHashMap;
use serde_json::Value;
use crate::util::nth;
use crate::util::statsapi::to_position_abbreviation;

#[derive(Clone)]
pub struct DefensiveSwitch {
    name: String,
    old_fielding_position: String,
    new_fielding_position: String,

    abbreviation: String,
    top: bool,
    inning: u8,
}

impl DefensiveSwitch {
    pub fn from_play(
        play: &Value,
        parent: &Value,
        abbreviation: &str,
        id_to_object: &FxHashMap<i64, Value>,
    ) -> Result<Self> {
        let name = id_to_object
            .get(
                &play["player"]["id"]
                    .as_i64()
                    .context("Could not find new player in defensive switch")?,
            )
            .context("New Player ID wasn't in the roaster for either team")?["fullName"]
            .as_str()
            .context("Could not find new player's name in defensive switch")?;
        let description = play["details"]["description"]
            .as_str()
            .context("Description must exist")?;
        Ok(Self {
            name: name.to_owned(),
            old_fielding_position: if description.contains(" remains in the game as ") {
                "PH".to_owned()
            } else {
                to_position_abbreviation(
                    description
                        .strip_prefix("Defensive switch from ")
                        .context("Defensive Switch didn't start correctly")?
                        .split_once(" to ")
                        .context("Defensive switch didn't have a `to` to split at")?
                        .0,
                )?
            },
            new_fielding_position: play["position"]["abbreviation"]
                .as_str()
                .context("Could not find player's position in defensive substitution")?
                .to_owned(),

            abbreviation: abbreviation.to_owned(),
            top: parent["about"]["isTopInning"].as_bool().context(
                "Could not find out if defensive switch was in the top or bottom of the inning",
            )?,
            inning: parent["about"]["inning"]
                .as_i64()
                .context("Could not find out defensive switch inning")? as u8,
        })
    }
}

impl Debug for DefensiveSwitch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            old_fielding_position,
            new_fielding_position,
            abbreviation,
            top,
            inning,
        } = self;

        if old_fielding_position == "PH" || old_fielding_position == "PR" {
            writeln!(f, "### [{abbreviation} Lineup Change] | {name} remains in the game as {new_fielding_position}.")?;
        } else {
            writeln!(f, "### [{abbreviation} Lineup Change] | {name} switches from {old_fielding_position} to {new_fielding_position}.")?;
        }
        writeln!(
            f,
            "> Inning: **{half} {n}**",
            half = if *top { "Top" } else { "Bot" },
            n = nth(*inning as usize)
        )?;
        writeln!(f, "")?;
        writeln!(f, "")?;

        Ok(())
    }
}