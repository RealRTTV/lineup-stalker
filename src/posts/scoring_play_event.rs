use std::fmt::{Debug, Display, Formatter};
use anyhow::{Result, Context};
use mlb_api::game::{ActionPlayDetails, Inning, InningHalf, Play, PlayEventCommon};
use mlb_api::meta::EventType;
use serde_json::Value;
use crate::util::nth;
use crate::util::statsapi::{BoldingDisplayKind, Score, ScoredRunner};

#[derive(Clone)]
pub struct ScoringPlayEvent {
    score: Score,
    inning: Inning,
    outs: u8,
    half: InningHalf,
    scores: Vec<ScoredRunner>,
    event: EventType,
}

impl ScoringPlayEvent {
    pub fn from_play(
        (details, _): (&ActionPlayDetails, &PlayEventCommon),
        play: &Play,
        home_abbreviation: &str,
        away_abbreviation: &str,
        all_player_names: &[&str],
        event: EventType,
    ) -> Self {
        let home_score = details.home_score;
        let away_score = details.away_score;
        let is_walkoff = play.about.inning_half == InningHalf::Bottom && *play.about.inning >= 9 && home_score > away_score;

        Self {
            inning: play.about.inning,
            outs: play.count.outs,
            half: play.about.inning_half,
            score: Score::new(away_abbreviation.to_owned(), details.away_score, home_abbreviation.to_owned(), details.home_score, 0, play.about.inning_half.bats(), BoldingDisplayKind::MostRecentlyScored, if is_walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None }),
            scores: ScoredRunner::from_description(&details.description, all_player_names),
            event,
        }
    }

    pub fn as_one_liner(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;

        let Self { score, inning, top, scores, .. } = self;
        let half = if *top { "Top" } else { "Bot" };
        let inning = nth(*inning as usize);
        write!(f, "{score} | {half} **{inning}**:", score = score.format_code_block())?;
        for score in scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

impl Debug for ScoringPlayEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{score:?} ({event})", score = self.score, event = self.event)?;
        writeln!(
            f,
            "{half} **{inning}**, **{outs}** out{out_suffix}.",
            half = self.half.three_char(),
            inning = nth(*self.inning),
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
        let Self { score, inning, half, scores, .. } = self;
        let half = half.three_char();
        let inning = nth(**inning);
        write!(f, "`{score}` | {half} **{inning}**:")?;
        for score in scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}