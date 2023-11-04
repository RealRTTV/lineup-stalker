use std::fmt::{Debug, Display, Formatter};
use anyhow::{Result, Context};
use serde_json::Value;
use crate::util::nth;
use crate::util::statsapi::{remap_score_event, Score};

pub struct ScoringPlay {
    inning: u8,
    top: bool,
    outs: u8,
    away_abbreviation: String,
    away_score: i64,
    home_abbreviation: String,
    home_score: i64,
    rbi: i64,
    scores: Vec<Score>,
    raw_event: String,
}

impl ScoringPlay {
    pub fn from_play(
        play: &Value,
        home_abbreviation: &str,
        away_abbreviation: &str,
        all_player_names: &[String],
    ) -> Result<Self> {
        Ok(Self {
            inning: play["about"]["inning"]
                .as_i64()
                .context("Could not find inning")? as u8,
            top: play["about"]["isTopInning"]
                .as_bool()
                .context("Could not find inning half")?,
            outs: play["count"]["outs"]
                .as_i64()
                .context("Could not find outs")? as u8,
            away_abbreviation: away_abbreviation.to_owned(),
            away_score: play["result"]["awayScore"]
                .as_i64()
                .context("Could not find away team's score")?,
            home_abbreviation: home_abbreviation.to_owned(),
            home_score: play["result"]["homeScore"]
                .as_i64()
                .context("Could not find away team's score")?,
            rbi: play["result"]["rbi"]
                .as_i64()
                .context("Could not find the RBI of the play")?,
            scores: {
                let description = play["result"]["description"]
                    .as_str()
                    .context("Play description didn't exist")?;
                let mut vec = Vec::new();
                let mut iter = description
                    .split_once(": ")
                    .map_or(description, |(_, x)| x)
                    .split("  ")
                    .map(str::trim)
                    .filter(|str| !str.is_empty());
                while let Some(value) = iter.next() {
                    let value = if all_player_names.iter().any(|name| value == *name) {
                        remap_score_event(
                            &format!(
                                "{value} {}",
                                iter.next().context("Play unexpectedly ended")?
                            ),
                            all_player_names,
                        )
                    } else {
                        remap_score_event(value, all_player_names)
                    };

                    let scoring = value.contains("scores.") || value.contains("homers") || value.contains("home run");
                    vec.push(Score::new(value, scoring));
                }
                vec
            },
            raw_event: play["result"]["eventType"]
                .as_str()
                .context("Could not find event type")?
                .to_owned(),
        })
    }
}

impl Display for ScoringPlay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { away_abbreviation, away_score, home_abbreviation, home_score, .. } = self;
        let half = if self.top { "Top" } else { "Bot" };
        let inning = nth(self.inning as usize);
        write!(f, "`{away_abbreviation} {away_score}-{home_score} {home_abbreviation}` | {half} **{inning}**:")?;
        for score in &self.scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

impl Debug for ScoringPlay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let walkoff = !self.top && self.inning >= 9 && self.home_score > self.away_score;
        let away_abbreviation = &*self.away_abbreviation;
        let away_score = self.away_score;
        let home_abbreviation = &*self.home_abbreviation;
        let home_score = self.home_score;
        let (away_bold, home_bold) = if self.top { ("**", "") } else { ("", "**") };
        let walkoff_bold = if walkoff { "**" } else { "" };
        let event = match &*self.raw_event {
            "single" => {
                if self.rbi == 1 {
                    "RBI single".to_owned()
                } else {
                    format!("{n}RBI single", n = self.rbi)
                }
            }
            "double" => {
                if self.rbi == 1 {
                    "RBI double".to_owned()
                } else {
                    format!("{n}RBI double", n = self.rbi)
                }
            }
            "triple" => {
                if self.rbi == 1 {
                    "RBI triple".to_owned()
                } else {
                    format!("{n}RBI triple", n = self.rbi)
                }
            }
            "home_run" => {
                if self.scores.iter().any(|score| score.play().contains("inside-the-park")) {
                    if self.scores.len() == 1 {
                        "HR".to_owned()
                    } else {
                        format!("{}HR", self.scores.len())
                    }
                } else {
                    if self.scores.len() == 1 {
                        "**HR**".to_owned()
                    } else {
                        format!("**{}HR**", self.scores.len())
                    }
                }
            }
            "grounded_into_double_play" => {
                if self.rbi == 1 {
                    "RBI double play".to_owned()
                } else {
                    format!("{n}RBI double play", n = self.rbi)
                }
            }
            "field_out" => {
                if self.rbi == 1 {
                    "RBI out".to_owned()
                } else {
                    format!("{n}RBI out", n = self.rbi)
                }
            }
            "sac_fly" => {
                if self.rbi == 1 {
                    "RBI sacrifice fly".to_owned()
                } else {
                    format!("{n}RBI sacrifice fly", n = self.rbi)
                }
            }
            "sac_bunt" => {
                if self.rbi == 1 {
                    "RBI sacrifice bunt".to_owned()
                } else {
                    format!("{n}RBI sacrifice bunt", n = self.rbi)
                }
            }
            "fielders_choice_out" => {
                if self.rbi == 1 {
                    "RBI Fielder's choice".to_owned()
                } else {
                    format!("{n}RBI Fielder's choice", n = self.rbi)
                }
            }
            "field_error" => "Error".to_owned(),
            "intent_walk" => "Bases loaded intentional walk".to_owned(),
            "balk" => "Balk".to_owned(),
            "walk" => "Bases loaded walk".to_owned(),
            "hit_by_pitch" => "Bases loaded HBP".to_owned(),
            event => format!("{n}RBI {event}", n = self.rbi),
        };

        writeln!(f, "{away_abbreviation} {away_bold}{away_score}{away_bold}-{home_bold}{home_score}{home_bold} {walkoff_bold}{home_abbreviation}{walkoff_bold} ({event})")?;
        writeln!(
            f,
            "{half} **{inning}**, **{outs}** out{out_suffix}.",
            half = if self.top { "Top" } else { "Bot" },
            inning = nth(self.inning as usize),
            outs = self.outs,
            out_suffix = if self.outs == 1 { "" } else { "s" }
        )?;
        for score in &self.scores {
            writeln!(f, "{score:?}")?
        }

        write!(f, "\n")?;

        Ok(())
    }
}