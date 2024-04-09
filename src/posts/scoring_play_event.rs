use std::fmt::{Debug, Display, Formatter};
use anyhow::{Result, Context};
use serde_json::Value;
use crate::util::nth;
use crate::util::statsapi::{remap_score_event, Score};

pub struct ScoringPlayEvent {
    away_abbreviation: String,
    away_score: i64,
    home_abbreviation: String,
    home_score: i64,
    inning: u8,
    outs: u8,
    top: bool,
    scores: Vec<Score>,
    event: &'static str,
}

impl ScoringPlayEvent {
    pub fn from_play(
        play: &Value,
        parent: &Value,
        home_abbreviation: &str,
        away_abbreviation: &str,
        all_player_names: &[String],
        event: &'static str,
    ) -> Result<Self> {
        Ok(Self {
            away_abbreviation: away_abbreviation.to_owned(),
            away_score: play["details"]["awayScore"]
                .as_i64()
                .context("Could not find away score")?,
            home_abbreviation: home_abbreviation.to_owned(),
            home_score: play["details"]["homeScore"]
                .as_i64()
                .context("Could not find home score")?,
            inning: parent["about"]["inning"]
                .as_i64()
                .context("Could not find inning")? as u8,
            outs: play["count"]["outs"]
                .as_i64()
                .context("Could not find outs")? as u8,
            top: parent["about"]["isTopInning"]
                .as_bool()
                .context("Could not find inning half")?,
            scores: {
                let description = play["details"]["description"]
                    .as_str()
                    .context("Could not get play description")?;
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

                    let scoring = value.contains(" scores.") || value.contains(" homers") || value.contains("home run");
                    vec.push(Score::new(value, scoring))
                }
                vec
            },
            event,
        })
    }
}

impl Debug for ScoringPlayEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let walkoff = !self.top && self.inning >= 9 && self.home_score > self.away_score;

        let away_abbreviation = &*self.away_abbreviation;
        let away_score = self.away_score;
        let home_abbreviation = &*self.home_abbreviation;
        let home_score = self.home_score;
        let (away_bold, home_bold) = if self.top { ("**", "") } else { ("", "**") };
        let walkoff_bold = if walkoff { "**" } else { "" };
        let event = self.event;

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
            writeln!(f, "{score:?}")?;
        }

        write!(f, "\n")?;

        Ok(())
    }
}

impl Display for ScoringPlayEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { away_abbreviation, away_score, home_abbreviation, home_score, inning, top, scores, .. } = self;
        let half = if *top { "Top" } else { "Bot" };
        let inning = nth(*inning as usize);
        write!(f, "`{away_abbreviation} {away_score}-{home_score} {home_abbreviation}` | {half} **{inning}**:")?;
        for score in scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}