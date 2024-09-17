use anyhow::{Result, anyhow};
use crate::util::ffi::{GetConsoleWindow, SetForegroundWindow};

pub mod pitching_substitution;
pub mod scoring_play;
pub mod offensive_substitution;
pub mod defensive_substitution;
pub mod scoring_play_event;
pub mod defensive_switch;

#[derive(Copy, Clone)]
pub enum Post {
    Lineup { has_lineup: bool, },
    ScoringPlay,
    PitchingSubstitution,
    OffensiveSubstitution,
    DefensiveSubstitution,
    DefensiveSwitch,
    PassedBall,
    StolenHome,
    FinalCard,
}

impl Post {
    #[inline]
    pub fn send(self, text: impl Into<String>) -> Result<()> {
        self.send_with_settings(text, true, true, false)
    }

    pub fn send_with_settings(self, text: impl Into<String>, stdout: bool, copy: bool, set_foreground_window: bool) -> Result<()> {
        let text = text.into();

        if stdout {
            println!("{}\n\n\n", text);
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
