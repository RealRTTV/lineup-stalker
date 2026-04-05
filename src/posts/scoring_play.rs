use crate::util::nth;
use crate::util::statsapi::{BoldingDisplayKind, Score, ScoredRunner};
use anyhow::{Context, Result};
use mlb_api::game::{Inning, InningHalf, Play};
use mlb_api::meta::EventType;
use std::fmt::{Debug, Display, Formatter};
use fxhash::FxHashMap;
use mlb_api::person::{Ballplayer, PersonId};
use crate::posts::Post;

#[derive(Clone)]
pub struct ScoringPlay {
    inning: Inning,
    half: InningHalf,
    outs: u8,
    score: Score,
    rbi: usize,
    scores: Vec<ScoredRunner>,
    event: EventType,
}

impl ScoringPlay {
    pub fn from_play(
        play: &Play,
        home_abbreviation: &str,
        away_abbreviation: &str,
        all_players: &FxHashMap<PersonId, Ballplayer<()>>,
    ) -> Result<Self> {
        let is_walkoff = play.about.inning_half == InningHalf::Bottom && *play.about.inning >= 9 && play.result.home_score > play.result.away_score;
        let details = play.result.completed_play_details.as_ref().context("Expected play to be complete")?;

        Ok(Self {
            inning: play.about.inning,
            half: play.about.inning_half,
            outs: play.count.outs,
            score: Score::new(away_abbreviation.to_owned(), play.result.away_score, home_abbreviation.to_owned(), play.result.home_score, 0, play.about.inning_half.bats(), BoldingDisplayKind::MostRecentlyScored, if is_walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None }),
            rbi: details.rbi,
            scores: ScoredRunner::from_description(&details.description, all_players),
            event: details.event
        })
    }

    pub fn as_one_liner(&self) -> OneLiner {
        OneLiner(self)
    }
}

#[must_use]
pub struct OneLiner<'a>(&'a ScoringPlay);

impl Display for OneLiner<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;

        write!(f, "{score} | {half} **{inning}**:", score = self.0.score.code_block(), half = self.0.half.three_char(), inning = nth(*self.0.inning))?;
        for score in &self.0.scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

impl Display for ScoringPlay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { score, .. } = self;
        let half = self.half.three_char();
        let inning = nth(*self.inning);
        write!(f, "`{score}` | {half} **{inning}**:")?;
        for score in &self.scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

impl Debug for ScoringPlay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{score:?} (", score = self.score)?;
        match self.event {
            EventType::HomeRun => if self.scores.iter().any(|score| score.play().contains("inside-the-park")) {
                if self.scores.len() == 1 {
                    write!(f, "HR")?
                } else {
                    write!(f, "{}HR", self.scores.len())?
                }
            } else {
                if self.scores.len() == 1 {
                    write!(f, "**HR**")?
                } else {
                    write!(f, "**{}HR**", self.scores.len())?
                }
            },
            EventType::FieldOut => {
                if self.rbi == 1 {
                    write!(f, "RBI groundout")?
                } else {
                    write!(f, "{n}RBI groundout", n = self.rbi)?
                }
            }
            EventType::ForceOut => {
                if self.rbi == 1 {
                    write!(f, "RBI forceout")?
                } else {
                    write!(f, "{n}RBI forceout", n = self.rbi)?
                }
            }
            EventType::FieldersChoiceFieldOut => {
                if self.rbi == 1 {
                    write!(f, "RBI Fielder's choice")?
                } else {
                    write!(f, "{n}RBI Fielder's choice", n = self.rbi)?
                }
            }
            EventType::FieldError => write!(f, "Error")?,
            EventType::IntentionalWalk => write!(f, "Bases loaded intentional walk")?,
            EventType::Walk => write!(f, "Bases loaded walk")?,
            EventType::HitByPitch => write!(f, "Bases loaded HBP")?,
            event => {
                if self.rbi == 1 {
                    write!(f, "RBI {event}", event = event.to_string().to_ascii_lowercase())?
                } else {
                    write!(f, "{n}RBI {event}", n = self.rbi, event = event.to_string().to_ascii_lowercase())?
                }
            },
        }
        writeln!(f, ")")?;
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

impl Post for ScoringPlay {}