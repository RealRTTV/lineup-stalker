use std::fmt::{Debug, Display, Formatter};
use anyhow::{Result, Context};
use serde_json::Value;
use crate::util::nth;
use crate::util::statsapi::{BoldingDisplayKind, Score, ScoredRunner};

#[derive(Clone)]
pub struct ScoringPlay {
    inning: u8,
    top: bool,
    outs: u8,
    score: Score,
    rbi: i64,
    scores: Vec<ScoredRunner>,
    raw_event: String,
}

impl ScoringPlay {
    pub fn from_play(
        play: &Value,
        home_abbreviation: &str,
        away_abbreviation: &str,
        all_player_names: &[String],
    ) -> Result<Self> {
        let inning = play["about"]["inning"]
            .as_i64()
            .context("Could not find inning (scoring play)")? as u8;
        let top = play["about"]["isTopInning"]
            .as_bool()
            .context("Could not find inning half")?;
        let home_score = play["result"]["homeScore"].as_i64().context("Could not find away team's score")? as usize;
        let away_score = play["result"]["awayScore"].as_i64().context("Could not find away team's score")? as usize;
        let walkoff = !top && inning >= 9 && home_score > away_score;

        Ok(Self {
            inning,
            top,
            outs: play["count"]["outs"]
                .as_i64()
                .context("Could not find outs")? as u8,
            score: Score::new(away_abbreviation.to_owned(), away_score, home_abbreviation.to_owned(), home_score, 0, !top, BoldingDisplayKind::MostRecentlyScored, if walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None }),
            rbi: play["result"]["rbi"]
                .as_i64()
                .context("Could not find the RBI of the play")?,
            scores: ScoredRunner::from_description(play["result"]["description"].as_str().context("Could not get play description")?, all_player_names),
            raw_event: play["result"]["eventType"]
                .as_str()
                .context("Could not find event type")?
                .to_owned(),
        })
    }

    pub fn one_liner(&self) -> String {
        use std::fmt::Write;

        let mut buf = String::new();
        let Self { score, .. } = self;
        let half = if self.top { "Top" } else { "Bot" };
        let inning = nth(self.inning as usize);
        write!(&mut buf, "{score} | {half} **{inning}**:", score = score.format_code_block()).unwrap_or(());
        for score in &self.scores {
            write!(&mut buf, " {score}").unwrap_or(());
        }
        buf
    }
}

impl Display for ScoringPlay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { score, .. } = self;
        let half = if self.top { "Top" } else { "Bot" };
        let inning = nth(self.inning as usize);
        write!(f, "`{score}` | {half} **{inning}**:")?;
        for score in &self.scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

impl Debug for ScoringPlay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
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
            "fielders_choice" => {
                "Fielder's Choice".to_owned()
            }
            "field_out" => {
                if self.rbi == 1 {
                    "RBI groundout".to_owned()
                } else {
                    format!("{n}RBI groundout", n = self.rbi)
                }
            }
            "force_out" => {
                if self.rbi == 1 {
                    "RBI forceout".to_owned()
                } else {
                    format!("{n}RBI forceout", n = self.rbi)
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

        writeln!(f, "{score:?} ({event})", score = self.score)?;
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