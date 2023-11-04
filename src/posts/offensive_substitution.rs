use std::fmt::{Debug, Formatter};
use anyhow::{Result, Context, anyhow};
use fxhash::FxHashMap;
use serde_json::Value;
use crate::util::nth;

pub enum OffensiveSubstitution {
    PinchRunner {
        old: String,
        new: String,

        abbreviation: String,
        top: bool,
        inning: u8,
    },
    PinchHitter {
        old: String,
        new: String,

        abbreviation: String,
        top: bool,
        inning: u8,
    },
}

impl OffensiveSubstitution {
    pub fn from_play(
        play: &Value,
        parent: &Value,
        abbreviation: &str,
        id_to_object: &FxHashMap<i64, Value>,
    ) -> Result<Self> {
        let old = id_to_object
            .get(
                &play["replacedPlayer"]["id"]
                    .as_i64()
                    .context("Could not find old player in offensive substitution")?,
            )
            .context("Old Player ID wasn't in the roaster for either team")?["person"]["fullName"]
            .as_str()
            .context("Could not find old player's name in offensive substitution")?
            .to_owned();
        let new = id_to_object
            .get(
                &play["player"]["id"]
                    .as_i64()
                    .context("Could not find new player in offensive substitution")?,
            )
            .context("New Player ID wasn't in the roaster for either team")?["person"]["fullName"]
            .as_str()
            .context("Could not find new player's name in offensive substitution")?
            .to_owned();
        let abbreviation = abbreviation.to_owned();
        let top = parent["about"]["isTopInning"]
            .as_bool()
            .context("Could not tell the inning half")?;
        let inning = parent["about"]["inning"]
            .as_i64()
            .context("Could not tell the inning")? as u8;
        match play["position"]["abbreviation"]
            .as_str()
            .context("Could not get offensive substitution position abbreviation")?
        {
            "PH" => Ok(Self::PinchHitter {
                old,
                new,

                abbreviation,
                top,
                inning,
            }),
            "PR" => Ok(Self::PinchRunner {
                old,
                new,

                abbreviation,
                top,
                inning,
            }),
            _ => Err(anyhow!("Invalid abbreviation ({:?}) for offensive substitution", &play["position"]["abbreviation"])),
        }
    }
}

impl Debug for OffensiveSubstitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OffensiveSubstitution::PinchRunner {
                old,
                new,
                abbreviation,
                top,
                inning,
            } => {
                writeln!(
                    f,
                    "### [{abbreviation} Lineup Change] | {new} pinch-running for {old}"
                )?;
                writeln!(
                    f,
                    "> Inning: **{half} {n}**",
                    half = if *top { "Top" } else { "Bot" },
                    n = nth(*inning as usize)
                )?;
                writeln!(f, "")?;
                writeln!(f, "")?;
            }
            OffensiveSubstitution::PinchHitter {
                old,
                new,
                abbreviation,
                top,
                inning,
            } => {
                writeln!(
                    f,
                    "### [{abbreviation} Lineup Change] | {new} pinch-hitting for {old}"
                )?;
                writeln!(
                    f,
                    "> Inning: **{half} {n}**",
                    half = if *top { "Top" } else { "Bot" },
                    n = nth(*inning as usize)
                )?;
                writeln!(f, "")?;
                writeln!(f, "")?;
            }
        }

        Ok(())
    }
}