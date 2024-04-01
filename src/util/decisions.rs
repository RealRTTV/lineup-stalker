use std::fmt::{Display, Formatter};
use serde_json::Value;
use anyhow::{Result, Context};
use crate::get;
use crate::util::last_name;

pub struct Decisions {
    winner: Win,
    loser: Loss,
    save: Option<Save>,
}

impl Decisions {
    pub fn new(response: &Value) -> Result<Self> {
        let game_id = response["gameData"]["game"]["pk"].as_i64().context("Could not get game id")?;

        Ok(Self {
            winner: {
                let pitcher_id = response["liveData"]["decisions"]["winner"]["id"].as_i64().context("Could not get winner's id")?;
                let winner = get(&format!("https://statsapi.mlb.com/api/v1/people/{pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?;
                let line = pitching_line(&winner, game_id).unwrap_or("**0.0** IP, **0** H, **0** ER, **0** BB, **0** K, 0 P".to_owned());
                let (wins, losses) = winner["people"][0]["stats"][0]["splits"].as_array().context("Could not get pitcher's splits")?.iter().fold((0, 0), |(wins, losses), split| (wins + split["stat"]["wins"].as_i64().unwrap_or(0), losses + split["stat"]["losses"].as_i64().unwrap_or(0)));
                Win {
                    name: last_name(winner["people"][0]["fullName"].as_str().context("Could not get pitcher's name")?).to_owned(),
                    wins,
                    losses,
                    line,
                }
            },
            loser: {
                let pitcher_id = response["liveData"]["decisions"]["loser"]["id"].as_i64().context("Could not get loser's id")?;
                let loser = get(&format!("https://statsapi.mlb.com/api/v1/people/{pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?;
                let line = pitching_line(&loser, game_id).unwrap_or("**0.0** IP, **0** H, **0** ER, **0** BB, **0** K, 0 P".to_owned());
                let (wins, losses) = loser["people"][0]["stats"][0]["splits"].as_array().context("Could not get pitcher's splits")?.iter().fold((0, 0), |(wins, losses), split| (wins + split["stat"]["wins"].as_i64().unwrap_or(0), losses + split["stat"]["losses"].as_i64().unwrap_or(0)));
                Loss {
                    name: last_name(loser["people"][0]["fullName"].as_str().context("Could not get pitcher's name")?).to_owned(),
                    wins,
                    losses,
                    line,
                }
            },
            save: {
                if let Some(pitcher_id) = response["liveData"]["decisions"]["save"]["id"].as_i64() {
                    let closer = get(&format!("https://statsapi.mlb.com/api/v1/people/{pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?;
                    let line = pitching_line(&closer, game_id).unwrap_or("**0.0** IP, **0** H, **0** ER, **0** BB, **0** K, 0 P".to_owned());
                    let saves = closer["people"][0]["stats"][0]["splits"].as_array().context("Could not get pitcher's splits")?.iter().fold(0, |saves, split| saves + split["stat"]["saves"].as_i64().unwrap_or(0));
                    Some(Save {
                        name: last_name(closer["people"][0]["fullName"].as_str().context("Could not get pitcher's name")?).to_owned(),
                        saves,
                        line,
                    })
                } else {
                    None
                }
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

fn pitching_line(splits: &Value, game_id: i64) -> Result<String> {
    let split = splits["people"][0]["stats"][0]["splits"].as_array().context("Expected player to have pitching splits")?.iter().find(|split| split["game"]["gamePk"].as_i64().is_some_and(|id| id == game_id)).context("Could not find this game being pitched")?;
    let stats = &split["stat"];
    let ip = stats["inningsPitched"].as_str().context("Could not get innings pitched")?;
    let hits = stats["hits"].as_i64().context("Could not get hits")?;
    let earned_runs = stats["earnedRuns"].as_i64().context("Could not get earned runs")?;
    let walks = stats["baseOnBalls"].as_i64().context("Could not get walks")?;
    let strikeouts = stats["strikeOuts"].as_i64().context("Could not get strikeouts")?;
    let pitches = stats["numberOfPitches"].as_i64().context("Could not get pitch count")?;
    let strikeout_surroundings = if strikeouts >= 12 { "__" } else { "" };

    Ok(format!("**{ip}** IP, **{hits}** H, **{earned_runs}** ER, **{walks}** BB, **{strikeout_surroundings}{strikeouts}** K{strikeout_surroundings}, **{pitches}** P"))
}

struct Win {
    name: String,
    wins: i64,
    losses: i64,
    line: String,
}

impl Display for Win {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (**{}**-{}) | {}", self.name, self.wins, self.losses, self.line)
    }
}

struct Loss {
    name: String,
    wins: i64,
    losses: i64,
    line: String,
}

impl Display for Loss {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}-**{}**) | {}", self.name, self.wins, self.losses, self.line)
    }
}

struct Save {
    name: String,
    saves: i64,
    line: String,
}

impl Display for Save {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (**{}**) | {}", self.name, self.saves, self.line)
    }
}

