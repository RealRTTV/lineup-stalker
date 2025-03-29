use std::fmt::{Debug, Display, Formatter};
use anyhow::{Result, Context};
use serde_json::Value;
use crate::util::nth;
use crate::util::statsapi::{BoldingDisplayKind, Score, ScoredRunner};

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
        let inning = parent["about"]["inning"]
            .as_i64()
            .context("Could not find inning (scoring play event)")? as u8;
        let top = parent["about"]["isTopInning"]
            .as_bool()
            .context("Could not find inning half")?;
        let home_score = parent["result"]["homeScore"].as_i64().context("Could not find home team's score")? as usize;
        let away_score = parent["result"]["awayScore"].as_i64().context("Could not find away team's score")? as usize;
        let walkoff = !top && inning >= 9 && home_score > away_score;

        Ok(Self {
            inning,
            outs: play["count"]["outs"]
                .as_i64()
                .context("Could not find outs")? as u8,
            top,
            score: Score::new(away_abbreviation.to_owned(), away_score, home_abbreviation.to_owned(), home_score, 0, !top, BoldingDisplayKind::MostRecentlyScored, if walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None }),
            scores: ScoredRunner::from_description(play["details"]["description"].as_str().context("Could not get play description")?, all_player_names),
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