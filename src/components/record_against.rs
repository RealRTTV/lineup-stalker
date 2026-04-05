use crate::util::statsapi::{BoldingDisplayKind, Score};
use core::fmt::{Display, Formatter};
use mlb_api::TeamSide;

#[derive(Clone)]
pub struct RecordAgainst {
    inner: Score,
}

impl RecordAgainst {
    pub fn new(our_abbreviation: &str, their_abbreviation: &str) -> Self {
        Self {
            inner: Score::new(our_abbreviation.to_owned(), 0, their_abbreviation.to_owned(), 0, 0, false, BoldingDisplayKind::MostRecentlyScored, BoldingDisplayKind::WinningTeam)
        }
    }

    pub fn win(&mut self) {
        self.inner.away_runs += 1;
        self.inner.who_scored = TeamSide::Away;
    }

    pub fn loss(&mut self) {
        self.inner.home_runs += 1;
        self.inner.who_scored = TeamSide::Home;
    }
}

impl Display for RecordAgainst {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { inner } = self;
        write!(f, "{inner:?}")
    }
}