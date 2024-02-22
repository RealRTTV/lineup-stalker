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

pub fn era(value: Value) -> Result<(f64, f64)> {
    let mut earned_runs = 0;
    let mut triple_innings_pitched = 0;
    let mut l7_earned_runs = 0;
    let mut l7_triple_innings_pitched = 0;
    let Some(arr) = value["people"][0]["stats"][0]["splits"].as_array() else { return Ok((0.0, 0.0)) };
    for (idx, split) in arr.iter()
        .rev()
        .enumerate()
    {
        let er = split["stat"]["earnedRuns"]
            .as_i64()
            .context("Pitcher doesn't have earnedRuns")?;
        let (int, rem) = split["stat"]["inningsPitched"]
            .as_str()
            .context("Pitcher doesn't have inningsPitched")?
            .split_at(1);
        let frac = rem.split_at(1).1;
        let tip = (int.as_bytes()[0] - b'0') as i64 * 3 + (frac.as_bytes()[0] - b'0') as i64;
        earned_runs += er;
        triple_innings_pitched += tip;
        if idx < 7 {
            l7_earned_runs += er;
            l7_triple_innings_pitched += tip;
        }
    }
    Ok(if triple_innings_pitched == 0 {
        (0.0, 0.0)
    } else {
        (
            (earned_runs * 9 * 3) as f64 / triple_innings_pitched as f64,
            (l7_earned_runs * 9 * 3) as f64 / l7_triple_innings_pitched as f64,
        )
    })
}

pub fn title(home: bool, home_full: &str, away_full: &str) -> String {
    if home {
        format!("{home_full} vs. {away_full}")
    } else {
        format!("{away_full} @ {home_full}")
    }
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
    let players = &previous_loadout["players"];
    let vec = match previous_loadout["battingOrder"].as_array() {
        Some(iter) => iter.iter().filter_map(|id| id.as_i64()).filter_map(|x| players[&format!("ID{x}")]["person"]["fullName"].as_str()).map(hide).collect::<Vec<String>>(),
        None => vec![hide("Babe Ruth"), hide("Shohei Ohtani"), hide("Kevin Gausman"), hide("Barry Bonds"), hide("Ronald Acuña Jr."), hide("Mariano Rivera"), hide("Melky Cabrera"), hide("Tony Castillo"), hide("Robin Yount")],
    };
    let [a, b, c, d, e, f, g, h, i] = vec.as_slice() else { return Err(anyhow!("Batting order was not 9 batters in length")) };
    writeln!(out, r"1 - {a} [\_\_] [.--- *|* .---]")?;
    writeln!(out, r"2 - {b} [\_\_] [.--- *|* .---]")?;
    writeln!(out, r"3 - {c} [\_\_] [.--- *|* .---]")?;
    writeln!(out, r"4 - {d} [\_\_] [.--- *|* .---]")?;
    writeln!(out, r"5 - {e} [\_\_] [.--- *|* .---]")?;
    writeln!(out, r"6 - {f} [\_\_] [.--- *|* .---]")?;
    writeln!(out, r"7 - {g} [\_\_] [.--- *|* .---]")?;
    writeln!(out, r"8 - {h} [\_\_] [.--- *|* .---]")?;
    writeln!(out, r"9 - {i} [\_\_] [.--- *|* .---]")?;
    Ok(())
}

pub fn lineup(root: &Value, prev: &Value, first_stat: HittingStat, second_stat: HittingStat) -> Result<String> {
    let mut players = Vec::new();
    for (key, player) in root["players"]
        .as_object()
        .context("Hitters didn't exist")?
    {
        let prev_player = prev["players"].get(key.as_str());
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
        if batting_order % 100 != 0 {
            continue;
        }
        let prev_batting_order = prev_player
            .and_then(|player| player["battingOrder"].as_str())
            .and_then(|x| x.parse::<i64>().ok());
        let (prev_position, position) = (
            prev_player.and_then(|x| x["allPositions"][0]["abbreviation"].as_str()),
            player["position"]["abbreviation"]
                .as_str()
                .context("Hitter's first position didn't exist")?,
        );
        let first_stat = first_stat.get(&player["seasonStats"]["batting"])?;
        let second_stat = second_stat.get(&player["seasonStats"]["batting"])?;
        let stats = format!("({first_stat} *|* {second_stat})");
        let changed_order = prev_batting_order.map_or(true, |prev_batting_order| {
            prev_batting_order != batting_order
        });
        let changed_order_surroundings = if changed_order { "**" } else { "" };
        let name_and_index = format!(
            "{changed_order_surroundings}{} - {name}{changed_order_surroundings}",
            batting_order / 100
        );
        let changed_position = prev_position
            .map_or(true, |prev_position| prev_position != position)
            || prev_batting_order.map_or(true, |x| x % 100 != 0);
        let changed_position_surroundings = if changed_position { "**" } else { "" };
        let position =
            format!("{changed_position_surroundings}[{position}]{changed_position_surroundings}");
        players.push((
            batting_order,
            format!("{name_and_index} {position} {stats}                                "),
        ));
    }
    let longest = players.iter().map(|(_, x)| x.len()).max().context("Batting order had at least one player")?;
    players.sort_by_key(|(x, _)| *x);
    let mut out = String::new();
    for (_, player) in players {
        writeln!(&mut out, "{player}{}", " ".repeat(longest - player.len()))?;
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

    let mut event = if event.contains(" on a fly ball") {
        event.replacen(" on a fly ball", "", 1)
    } else if event.contains(" on a sharp fly ball") {
        event.replacen(" on a sharp fly ball", "", 1)
    } else if event.contains(" on a ground ball") {
        event.replacen(" on a ground ball", "", 1)
    } else if event.contains(" on a sharp ground ball") {
        event.replacen(" on a sharp ground ball", "", 1)
    } else if event.contains(" on a line drive") {
        event.replacen(" on a line drive", "", 1)
    } else if event.contains(" on a sharp line drive") {
        event.replacen(" on a sharp line drive", "", 1)
    } else {
        event.to_owned()
    };

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
