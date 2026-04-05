use std::cell::LazyCell;
use crate::util::ffi::{GetConsoleWindow, SetForegroundWindow};
use anyhow::{anyhow, Result};
use std::fmt::Display;

pub mod pitching_line;
pub mod scoring_play;
pub mod scoring_play_event;
pub mod lineup;
pub mod final_card;

pub trait Post: Display {
    fn send(&self) -> Result<()> {
        self.send_with_settings(true, true, false)
    }

    fn send_with_settings(&self, stdout: bool, copy: bool, set_foreground_window: bool) -> Result<()> {
        let text = LazyCell::new(move || self.to_string());

        if stdout {
            println!("{}\n\n\n", text.to_string());
            let _ = std::io::Write::flush(&mut std::io::stdout())?;
        }

        if copy {
            cli_clipboard::set_contents(text.to_string()).map_err(|_| anyhow!("Failed to set clipboard"))?;
        }

        if set_foreground_window {
            unsafe { SetForegroundWindow(GetConsoleWindow().cast()); }
        }

        Ok(())
    }
}

