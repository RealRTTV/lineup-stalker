use std::fmt::{Display, Formatter};
use crate::util::decisions::Decisions;
use crate::util::line_score::LineScore;
use crate::util::next_game::NextGame;
use crate::util::record_against::RecordAgainst;
use crate::util::standings::Standings;
use crate::util::statsapi::Score;

#[derive(Clone)]
pub struct FinalCard {
    score: Score,
    standings: Standings,
    record: RecordAgainst,
    next_game: Option<NextGame>,
    masterpieces: String,
    line_score: LineScore,
    scoring_plays: Vec<String>,
    decisions: Option<Decisions>,
}

impl FinalCard {
    pub fn new(score: Score, standings: Standings, record: RecordAgainst, next_game: Option<NextGame>, masterpieces: String, line_score: LineScore, scoring_plays: Vec<String>, decisions: Option<Decisions>) -> Self {
        Self {
            score,
            standings,
            record,
            next_game,
            masterpieces,
            line_score,
            scoring_plays,
            decisions,
        }
    }
}

impl Display for FinalCard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { score, standings, record, next_game, masterpieces, line_score, scoring_plays, decisions } = self;
        writeln!(f, "## Final Score")?;
        writeln!(f, "{score:?}")?;
        writeln!(f, "Standings: {standings}")?;
        writeln!(f, "Record Against: {record}")?;
        if let Some(next_game) = next_game {
            writeln!(f, "Next Game: {next_game}")?;
        }
        write!(f, "{masterpieces}")?;
        writeln!(f, "### __Line Score__")?;
        writeln!(f, "{line_score}")?;
        writeln!(f, "### __Scoring Plays__")?;
        writeln!(f, "{}", scoring_plays.join("\n"))?;
        if let Some(decisions) = decisions {
            writeln!(f, "### __Pitcher Decisions__")?;
            writeln!(f, "{decisions}")?;
        }
        write!(f, "> ")?;

        Ok(())
    }
}
