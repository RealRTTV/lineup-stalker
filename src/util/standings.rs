use core::num::NonZeroUsize;
use core::fmt::Formatter;
use core::fmt::Display;

#[derive(Clone)]
pub struct Standings {
    wins: i64,
    losses: i64,
    streak: Option<(bool, NonZeroUsize)>,
}

impl Standings {
    pub fn new() -> Self {
        Self {
            wins: 0,
            losses: 0,
            streak: None,
        }
    }

    pub fn streak_older_win(&mut self) -> bool {
        self.wins += 1;
        if let Some((true, n)) = &mut self.streak {
            *n = n.saturating_add(1);
            true
        } else if self.streak.is_none() {
            self.streak = Some((true, NonZeroUsize::MIN));
            true
        } else {
            false
        }
    }

    pub fn streak_older_loss(&mut self) -> bool {
        self.losses += 1;
        if let Some((false, n)) = &mut self.streak {
            *n = n.saturating_add(1);
            true
        } else if self.streak.is_none() {
            self.streak = Some((false, NonZeroUsize::MIN));
            true
        } else {
            false
        }
    }

    pub fn win(&mut self) {
        self.wins += 1;
        if let Some((true, n)) = &mut self.streak {
            *n = n.saturating_add(1);
        } else {
            self.streak = Some((true, NonZeroUsize::MIN));
        }
    }

    pub fn loss(&mut self) {
        self.losses += 1;
        if let Some((false, n)) = &mut self.streak {
            *n = n.saturating_add(1);
        } else {
            self.streak = Some((false, NonZeroUsize::MIN));
        }
    }
}

impl Display for Standings {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let (wins, losses) = (self.wins, self.losses);
        if let Some((kind, streak)) = self.streak {
            if kind {
                write!(f, "**{wins}**-{losses} (__W{streak}__)")
            } else {
                write!(f, "{wins}-**{losses}** (__L{streak}__)")
            }
        } else {
            write!(f, "{wins}-{losses} (__N/A__)")
        }
    }
}