use anyhow::{Context, Result};
use mlb_api::game::{Boxscore, PlayerWithGameData, TeamWithGameData};
use mlb_api::stats::CountingStat;
use std::fmt::{Display, Formatter};
use mlb_api::person::PersonId;
use mlb_api::{HomeAway, TeamSide};
use crate::posts::pitching_line::PitchingLine;

#[derive(Clone)]
pub struct Decisions {
    winner: Win,
    loser: Loss,
    save: Option<Save>,
}

impl Decisions {
    pub fn new(decisions: &mlb_api::game::Decisions, boxscore: &Boxscore) -> Result<Self> {
        fn get_person_with_team(boxscore: &Boxscore, id: PersonId) -> Result<(&PlayerWithGameData, &TeamWithGameData)> {
            let HomeAway { home, away } = boxscore.teams.as_ref().map(|team| team.players.get(&id).map(|player| (player, team)));
            home.or(away).context("Expected the winner to play in the game")
        }

        let (winner, winners_team) = get_person_with_team(boxscore, decisions.winner.as_ref().context("Expected a winner")?.id)?;
        let (loser, losers_team) = get_person_with_team(boxscore, decisions.loser.as_ref().context("Expected a loser")?.id)?;

        Ok(Self {
            winner: {
                Win {
                    name: winner.boxscore_name.clone(),
                    wins: winner.season_stats.pitching.wins.unwrap_or_default(),
                    losses: winner.season_stats.pitching.losses.unwrap_or_default(),
                    line: PitchingLine::from_stats(&winner.stats.pitching, winners_team.pitchers.len() == 1, false),
                }
            },
            loser: {
                Loss {
                    name: loser.boxscore_name.clone(),
                    wins: loser.season_stats.pitching.wins.unwrap_or_default(),
                    losses: loser.season_stats.pitching.losses.unwrap_or_default(),
                    line: PitchingLine::from_stats(&loser.stats.pitching, losers_team.pitchers.len() == 1, false),
                }
            },
            save: {
                decisions.save.as_ref().and_then(|person| boxscore.find_player_with_game_data(person.id)).map(|closer| {
                    Save {
                        name: closer.boxscore_name.clone(),
                        saves: closer.season_stats.pitching.saves.unwrap_or_default(),
                        line: PitchingLine::from_stats(&closer.stats.pitching, false, false),
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

fn pitching_line() -> PitchingLine {

}

#[derive(Clone)]
struct Win {
    name: String,
    wins: CountingStat,
    losses: CountingStat,
    line: PitchingLine,
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
    line: PitchingLine,
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
    line: PitchingLine,
}

impl Display for Save {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (**{}**) | {}", self.name, self.saves, self.line)
    }
}

