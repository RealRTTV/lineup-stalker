use std::fmt::Display;
use anyhow::{Result, anyhow};
use crate::posts::defensive_substitution::DefensiveSubstitution;
use crate::posts::defensive_switch::DefensiveSwitch;
use crate::posts::final_card::FinalCard;
use crate::posts::lineup::Lineup;
use crate::posts::offensive_substitution::OffensiveSubstitution;
use crate::posts::pitching_substitution::PitchingSubstitution;
use crate::posts::scoring_play::ScoringPlay;
use crate::posts::scoring_play_event::ScoringPlayEvent;
use crate::util::ffi::{GetConsoleWindow, SetForegroundWindow};

pub mod pitching_substitution;
pub mod scoring_play;
pub mod offensive_substitution;
pub mod defensive_substitution;
pub mod scoring_play_event;
pub mod defensive_switch;
pub mod lineup;
pub mod final_card;

#[derive(Clone)]
pub enum Post {
    Lineup(Lineup),
    ScoringPlay(ScoringPlay),
    PitchingSubstitution(PitchingSubstitution),
    OffensiveSubstitution(OffensiveSubstitution),
    DefensiveSubstitution(DefensiveSubstitution),
    DefensiveSwitch(DefensiveSwitch),
    PassedBall(ScoringPlayEvent),
    StolenHome(ScoringPlayEvent),
    FinalCard(FinalCard),
}

impl Post {
    #[inline]
    pub fn send(&self) -> Result<()> {
        self.send_with_settings(true, true, false)
    }

    pub fn send_with_settings(&self, stdout: bool, copy: bool, set_foreground_window: bool) -> Result<()> {
        let text = self.to_string();

        if stdout {
            println!("{text}\n\n\n");
            let _ = std::io::Write::flush(&mut std::io::stdout())?;
        }

        if copy {
            cli_clipboard::set_contents(text).map_err(|_| anyhow!("Failed to set clipboard"))?;
        }

        if set_foreground_window {
            unsafe { SetForegroundWindow(GetConsoleWindow().cast()); }
        }

        Ok(())
    }
}

impl Display for Post {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lineup(inner) => write!(f, "{inner}"),
            Self::ScoringPlay(inner) => write!(f, "{inner:?}"),
            Self::PitchingSubstitution(inner) => write!(f, "{inner:?}"),
            Self::OffensiveSubstitution(inner) => write!(f, "{inner:?}"),
            Self::DefensiveSubstitution(inner) => write!(f, "{inner:?}"),
            Self::DefensiveSwitch(inner) => write!(f, "{inner:?}"),
            Self::PassedBall(inner) => write!(f, "{inner:?}"),
            Self::StolenHome(inner) => write!(f, "{inner:?}"),
            Self::FinalCard(inner) => write!(f, "{inner}"),
        }
    }
}
