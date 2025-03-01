use std::fmt::{Debug, Display, Formatter};
use anyhow::{Result, Context};
use serde_json::Value;
use crate::util::nth;
use crate::util::statsapi::{remap_score_event, BoldingDisplayKind, Score, ScoredRunner};

#[derive(Clone)]
pub struct ScoringPlayEvent {
    score: Score,
    inning: u8,
    outs: u8,
    top: bool,
    scores: Vec<ScoredRunner>,
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
        let inning = play["about"]["inning"]
            .as_i64()
            .context("Could not find inning")? as u8;
        let top = play["about"]["isTopInning"]
            .as_bool()
            .context("Could not find inning half")?;
        let home_score = play["result"]["homeScore"].as_i64().context("Could not find away team's score")? as usize;
        let away_score = play["result"]["awayScore"].as_i64().context("Could not find away team's score")? as usize;
        let walkoff = !top && inning >= 9 && home_score > away_score;

        Ok(Self {
            inning: parent["about"]["inning"]
                .as_i64()
                .context("Could not find inning")? as u8,
            outs: play["count"]["outs"]
                .as_i64()
                .context("Could not find outs")? as u8,
            top: parent["about"]["isTopInning"]
                .as_bool()
                .context("Could not find inning half")?,
            score: Score::new(away_abbreviation.to_owned(), away_score, home_abbreviation.to_owned(), home_score, 0, !top, BoldingDisplayKind::MostRecentlyScored, if walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None }),
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
                    vec.push(ScoredRunner::new(value, scoring))
                }
                vec
            },
            event,
        })
    }

    pub fn one_liner(&self) -> String {
        use std::fmt::Write;

        let mut buf = String::new();
        let Self { score, inning, top, scores, .. } = self;
        let half = if *top { "Top" } else { "Bot" };
        let inning = nth(*inning as usize);
        write!(&mut buf, "{score} | {half} **{inning}**:", score = score.format_code_block()).unwrap_or(());
        for score in scores {
            write!(&mut buf, " {score}").unwrap_or(());
        }
        buf
    }
}

impl Debug for ScoringPlayEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{score:?} ({event})", score = self.score, event = self.event)?;
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
        let Self { score, inning, top, scores, .. } = self;
        let half = if *top { "Top" } else { "Bot" };
        let inning = nth(*inning as usize);
        write!(f, "`{score}` | {half} **{inning}**:")?;
        for score in scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}