use std::fmt::{Display, Formatter};
use anyhow::{Result, Context};
use mlb_api::game::Boxscore;
use mlb_api::stats::CountingStat;
use mlb_api::stats::raw::pitching;
use mlb_api::stats::wrappers::WithNone;

#[derive(Clone)]
pub struct Decisions {
    winner: Win,
    loser: Loss,
    save: Option<Save>,
}

impl Decisions {
    pub fn new(decisions: &mlb_api::game::Decisions, boxscore: &Boxscore) -> Result<Self> {
        let winner = boxscore.find_player_with_game_data(decisions.winner.as_ref().context("Expected a winner")?.id).context("Expected the winner to play in the game")?;
        let loser = boxscore.find_player_with_game_data(decisions.loser.as_ref().context("Expected a loser")?.id).context("Expected the loser to play in the game")?;
        
        Ok(Self {
            winner: {
                Win {
                    name: winner.boxscore_name.clone(),
                    wins: winner.season_stats.pitching.wins.unwrap_or_default(),
                    losses: winner.season_stats.pitching.losses.unwrap_or_default(),
                    line: pitching_line(&winner.stats.pitching),
                }
            },
            loser: {
                Loss {
                    name: loser.boxscore_name.clone(),
                    wins: loser.season_stats.pitching.wins.unwrap_or_default(),
                    losses: loser.season_stats.pitching.losses.unwrap_or_default(),
                    line: pitching_line(&loser.stats.pitching),
                }
            },
            save: {
                decisions.save.as_ref().and_then(|person| boxscore.find_player_with_game_data(person.id)).map(|closer| {
                    Save {
                        name: closer.boxscore_name.clone(),
                        saves: closer.season_stats.pitching.saves.unwrap_or_default(),
                        line: pitching_line(&closer.stats.pitching),
                    }
                })
            },
        })
    }
}

impl Display for Decisions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "**Win**: {}", self.winner)?;
        writeln!(f, "**Loss**: {}", self.loser)?;
        if let Some(save) = self.save.as_ref() {
            writeln!(f, "**Save**: {}", save)?;
        }
        Ok(())
    }
}

fn pitching_line(stats: &WithNone<pitching::__BoxscoreStatsData>) -> String {
    let ip = stats.innings_pitched.unwrap_or_default();
    let hits = stats.hits.unwrap_or_default();
    let earned_runs = stats.earned_runs.unwrap_or_default();
    let walks = stats.base_on_balls.unwrap_or_default();
    let strikeouts = stats.strikeouts.unwrap_or_default();
    let pitches = stats.number_of_pitches.unwrap_or_default();
    let strikeout_surroundings = if strikeouts >= 12 { "__" } else { "" };

    format!("**{ip}** IP, **{hits}** H, **{earned_runs}** ER, **{walks}** BB, **{strikeout_surroundings}{strikeouts}** K{strikeout_surroundings}, **{pitches}** P")
}

#[derive(Clone)]
struct Win {
    name: String,
    wins: CountingStat,
    losses: CountingStat,
    line: String,
}

impl Display for Win {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (**{}**-{}) | {}", self.name, self.wins, self.losses, self.line)
    }
}

#[derive(Clone)]
struct Loss {
    name: String,
    wins: CountingStat,
    losses: CountingStat,
    line: String,
}

impl Display for Loss {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}-**{}**) | {}", self.name, self.wins, self.losses, self.line)
    }
}

#[derive(Clone)]
struct Save {
    name: String,
    saves: CountingStat,
    line: String,
}

impl Display for Save {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (**{}**) | {}", self.name, self.saves, self.line)
    }
}

