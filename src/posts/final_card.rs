use std::fmt::{Display, Formatter};
use crate::posts::components::decisions::Decisions;
use crate::posts::components::line_score::LineScore;
use crate::posts::components::next_game::NextGame;
use crate::posts::components::record_against::RecordAgainst;
use crate::posts::components::standings::Standings;
use crate::util::statsapi::Score;

#[derive(Clone)]
pub struct FinalCard {
    score: Score,
    standings: Option<Standings>,
    record_text: &'static str,
    record: RecordAgainst,
    next_game: Option<NextGame>,
    masterpieces: String,
    line_score: LineScore,
    scoring_plays: Vec<String>,
    decisions: Option<Decisions>,
}

impl FinalCard {
    pub fn new(score: Score, standings: Option<Standings>, record_text: &'static str, record: RecordAgainst, next_game: Option<NextGame>, masterpieces: String, line_score: LineScore, scoring_plays: Vec<String>, decisions: Option<Decisions>) -> Self {
        Self {
            score,
            standings,
            record_text,
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
        let Self { score, standings, record_text, record, next_game, masterpieces, line_score, scoring_plays, decisions } = self;
        writeln!(f, "## Final Score")?;
        writeln!(f, "{score:?}")?;
        if let Some(standings) = standings {
            writeln!(f, "Standings: {standings}")?;
        }
        writeln!(f, "{record_text}: {record}")?;
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
