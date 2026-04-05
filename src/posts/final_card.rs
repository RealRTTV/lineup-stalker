use crate::components::decisions::Decisions;
use crate::components::linescore::LineScore;
use crate::components::next_game::NextGame;
use crate::components::record_against::RecordAgainst;
use crate::components::standings::Standings;
use crate::util::statsapi::Score;
use std::fmt::{Display, Formatter};
use crate::components::pitching_masterpiece::PitchingMasterpiece;
use crate::posts::Post;

#[derive(Clone)]
pub struct FinalCard {
    pub score: Score,
    pub standings: Option<Standings>,
    pub record_text: &'static str,
    pub record: RecordAgainst,
    pub next_game: Option<NextGame>,
    pub pitching_masterpiece: Option<PitchingMasterpiece>,
    pub linescore: LineScore,
    pub scoring_plays: String,
    pub decisions: Option<Decisions>,
}

impl Display for FinalCard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { score, standings, record_text, record, next_game, pitching_masterpiece: pitching_masterpiece, linescore: line_score, scoring_plays, decisions } = self;
        writeln!(f, "## Final Score")?;
        writeln!(f, "{score:?}")?;
        if let Some(standings) = standings {
            writeln!(f, "Standings: {standings}")?;
        }
        writeln!(f, "{record_text}: {record}")?;
        if let Some(next_game) = next_game {
            writeln!(f, "Next Game: {next_game}")?;
        }
        if let Some(pitching_masterpiece) = self.pitching_masterpiece {
            writeln!(f, "{pitching_masterpiece}")?;
        }
        writeln!(f, "### __Line Score__")?;
        writeln!(f, "{line_score}")?;
        writeln!(f, "### __Scoring Plays__")?;
        writeln!(f, "{scoring_plays}")?;
        if let Some(decisions) = decisions {
            writeln!(f, "### __Pitcher Decisions__")?;
            writeln!(f, "{decisions}")?;
        }
        write!(f, "> ")?;

        Ok(())
    }
}

impl Post for FinalCard {}
