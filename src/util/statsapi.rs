use core::fmt::{Debug, Display, Formatter, Write};
use anyhow::{Result, Context, anyhow};
use serde_json::Value;
use crate::util::hide;
use crate::util::stat::HittingStat;

pub struct Score {
    play: String,
    scoring: bool,
}

impl Score {
    pub fn new(value: String, scoring: bool) -> Self {
        Self {
            play: value,
            scoring,
        }
    }

    pub fn play(&self) -> &str {
        &self.play
    }
}

impl Debug for Score {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.scoring {
            write!(f, "> **{}**", self.play)
        } else {
            write!(f, "> {}", self.play)
        }
    }
}

impl Display for Score {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.play)
    }
}

pub fn pitching_stats(value: Value) -> Result<(f64, f64, String)> {
    let hand = value["people"][0]["pitchHand"]["code"].as_str().context("Could not get pitcher's hand")?.to_owned();
    let mut total_earned_runs = 0;
    let mut total_outs = 0;
    let Some(arr) = value["people"][0]["stats"][0]["splits"].as_array() else { return Ok((0.0, 0.0, hand)) };
    for split in arr.iter().rev() {
        total_earned_runs += split["stat"]["earnedRuns"].as_i64().context("Pitcher doesn't have earnedRuns")?;
        total_outs += split["stat"]["outs"].as_i64().context("Could not get pitchers outs")?;
    }
    Ok(if total_outs == 0 {
        (0.0, 0.0, hand)
    } else {
        (
            (total_earned_runs * 9 * 3) as f64 / total_outs as f64,
            (total_outs / 3) as f64 + (total_outs % 3) as f64 / 10.0,
            hand,
        )
    })
}

pub fn to_position_abbreviation(s: &str) -> Result<String> {
    let s = s.to_ascii_lowercase();
    Ok(match &*s {
        "pitcher" => "P",
        "catcher" => "C",
        "first baseman" | "first base" => "1B",
        "second baseman" | "second base" => "2B",
        "third baseman" | "third base" => "3B",
        "shortstop" => "SS",
        "left fielder" | "left field" => "LF",
        "center fielder" | "center field" => "CF",
        "right fielder" | "right field" => "RF",
        _ => return Err(anyhow!("Invalid fielding position '{s}'")),
    }.to_owned())
}

pub fn real_abbreviation(parent: &Value) -> Result<String> {
    let original = parent["abbreviation"]
        .as_str()
        .context("Team didn't have an abbreviated name")?;
    if original.len() == 3 {
        return Ok(original.to_owned());
    }
    let location = parent["franchiseName"]
        .as_str()
        .context("Team didn't have a location name")?;
    let team_name = parent["clubName"]
        .as_str()
        .context("Team didn't have a team name")?;
    let acronym = location
        .split(' ')
        .chain(team_name.split(' '))
        .filter_map(|s| s.chars().nth(0))
        .collect::<String>();
    if acronym.len() == 3 {
        return Ok(acronym);
    }
    let file_code = parent["teamCode"]
        .as_str()
        .context("Team didn't have a file code")?;
    if file_code.len() == 3 {
        return Ok(file_code.to_ascii_uppercase());
    }
    let team_code = parent["teamCode"]
        .as_str()
        .context("Team didn't have a team code")?;
    if team_code.len() == 3 {
        return Ok(team_code.to_ascii_uppercase());
    }
    Ok(acronym)
}

pub fn write_last_lineup_underscored(out: &mut String, previous_loadout: &Value) -> Result<()> {
    let default_batting_order = vec![hide("Babe Ruth"), hide("Shohei Ohtani"), hide("Kevin Gausman"), hide("Barry Bonds"), hide("Ronald Acuña Jr."), hide("Mariano Rivera"), hide("Jacob deGrom"), hide("Ichiro Suzuki"), hide("Dave Stieb")];
    let players = &previous_loadout["players"];
    let vec = match previous_loadout["battingOrder"].as_array() {
        Some(iter) => iter.iter().filter_map(|id| id.as_i64()).filter_map(|x| players[&format!("ID{x}")]["person"]["fullName"].as_str()).map(hide).collect::<Vec<String>>(),
        None => default_batting_order.clone(),
    };
    let lineup = if vec.len() == 9 { vec } else { default_batting_order };
    let [a, b, c, d, e, f, g, h, i] = lineup.as_slice() else { return Err(anyhow!("Batting order was not 9 batters in length")) };
    writeln!(out, r"`1` | **\_\_** {a} [.--- *|* .---]")?;
    writeln!(out, r"`2` | **\_\_** {b} [.--- *|* .---]")?;
    writeln!(out, r"`3` | **\_\_** {c} [.--- *|* .---]")?;
    writeln!(out, r"`4` | **\_\_** {d} [.--- *|* .---]")?;
    writeln!(out, r"`5` | **\_\_** {e} [.--- *|* .---]")?;
    writeln!(out, r"`6` | **\_\_** {f} [.--- *|* .---]")?;
    writeln!(out, r"`7` | **\_\_** {g} [.--- *|* .---]")?;
    writeln!(out, r"`8` | **\_\_** {h} [.--- *|* .---]")?;
    writeln!(out, r"`9` | **\_\_** {i} [.--- *|* .---]")?;
    Ok(())
}

pub fn lineup(root: &Value, first_stat: HittingStat, second_stat: HittingStat, show_stats: bool, team_name: &str) -> Result<String> {
    let mut players = Vec::new();
    for (_, player) in root["players"]
        .as_object()
        .context("Hitters didn't exist")?
    {
        let person = player["person"]
            .as_object()
            .context("Hitter's 'person' didn't exist")?;
        let name = person["fullName"]
            .as_str()
            .context("Hitter's name didn't exist")?;
        let Some(batting_order) = player["battingOrder"]
            .as_str()
            .and_then(|x| x.parse::<i64>().ok())
            else {
                continue;
            };
        if batting_order % 100 != 0 { continue; }
        let position = player["position"]["abbreviation"].as_str().context("Hitter's first position didn't exist")?;
        let first_stat = first_stat.get(&player["seasonStats"]["batting"], team_name)?;
        let second_stat = second_stat.get(&player["seasonStats"]["batting"], team_name)?;
        let stats = format!(" ({first_stat} *|* {second_stat})");
        players.push((
            batting_order,
            format!("`{}` | **{position}** {name}{stats}", batting_order / 100, stats = if show_stats { stats } else { String::new() }),
        ));
    }
    players.sort_by_key(|(x, _)| *x);
    let mut out = String::new();
    for (_, player) in players {
        writeln!(&mut out, "{player}")?;
    }
    Ok(out)
}

pub fn remap_score_event(event: &str, all_player_names: &[String]) -> String {
    fn remove_prefix<'a>(s: &'a str, prefixes: impl Iterator<Item = &'a str>) -> Option<&'a str> {
        for prefix in prefixes {
            if let Some(s) = s.strip_prefix(prefix) {
                return Some(s);
            }
        }
        None
    }

    let mut event = event
        .replacen(" on a fly ball", "", 1)
        .replacen(" on a sharp fly ball", "", 1)
        .replacen(" on a ground ball", "", 1)
        .replacen(" on a sharp ground ball", "", 1)
        .replacen(" on a line drive", "", 1)
        .replacen(" on a sharp line drive", "", 1)
        .replacen(" down the left-field line", "", 1)
        .replacen(" down the right-field line", "", 1);

    loop {
        event = if let Some((left, right)) = event.split_once(" left fielder") {
            let Some(right) = remove_prefix(
                right.trim_start(),
                all_player_names.iter().map(String::as_str),
            ) else {
                break;
            };
            format!("{left} left field{right}")
        } else if let Some((left, right)) = event.split_once(" center fielder") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str), ) else { break; };
            format!("{left} center field{right}")
        } else if let Some((left, right)) = event.split_once(" right fielder") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
            format!("{left} right field{right}")
        } else if let Some((left, right)) = event.split_once(" first baseman") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
            format!("{left} first base{right}")
        } else if let Some((left, right)) = event.split_once(" second baseman") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
            format!("{left} second base{right}")
        } else if let Some((left, right)) = event.split_once(" third baseman") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
            format!("{left} third base{right}")
        } else if let Some((left, right)) = event.split_once(" catcher") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
            format!("{left} catcher{right}")
        } else if let Some((left, right)) = event.split_once(" pitcher") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
            format!("{left} pitcher{right}")
        } else if let Some((left, right)) = event.split_once(" shortstop") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
            format!("{left} shortstop{right}")
        } else {
            break;
        }
    }

    event.replace("1st", "first").replace("2nd", "second").replace("3rd", "third")
}
