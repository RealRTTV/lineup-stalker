use core::fmt::{Debug, Display, Formatter};
use std::cmp::Ordering;
use std::mem::MaybeUninit;
use anyhow::{Result, Context, anyhow};
use serde_json::Value;
use crate::util::hide;
use crate::util::hitting::HitterLineupEntry;
use crate::util::stat::HittingStat;
use crate::util::team_stats_log::TeamStatsLog;

#[derive(Clone)]
pub struct ScoredRunner {
    play: String,
    scoring: bool,
}

impl ScoredRunner {
    pub fn new(value: String, scoring: bool) -> Self {
        Self {
            play: value,
            scoring,
        }
    }

    pub fn play(&self) -> &str {
        &self.play
    }
    
    pub fn from_description(description: &str, all_player_names: &[String]) -> Vec<Self> {
        let scores = description
            .split_once(": ")
            .map_or(description, |(_, x)| x);
        // spec changed
        let mut iter = if scores.contains("  ") {
            scores
                .split("  ")
                .map(str::trim)
                .filter(|str| !str.is_empty())
                .map(str::to_owned)
                .collect::<Vec<String>>()
        } else {
            scores
                .split(". ")
                .map(str::trim)
                .filter(|str| !str.is_empty())
                .map(str::to_owned)
                .map(|s| if s.ends_with('.') { s } else { s + "." })
                .collect::<Vec<String>>()
        }.into_iter();
        let mut vec = Vec::new();
        while let Some(value) = iter.next() {
            // names with a . in them (ex: Vladimir Guerrero Jr.) are broken in the formatter, so it has to be patched
            let value = if all_player_names.iter().any(|name| value == *name) && let Some(next) = iter.next() {
                remap_score_event(&format!("{value} {next}"), all_player_names, )
            } else {
                remap_score_event(&value, all_player_names)
            };

            let scoring = value.contains("scores.") || value.contains("homers") || value.contains("home run") || value.contains("grand slam");
            vec.push(ScoredRunner::new(value, scoring));
        }
        vec
    }
}

impl Debug for ScoredRunner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.scoring {
            write!(f, "> **{}**", self.play)
        } else {
            write!(f, "> {}", self.play)
        }
    }
}

impl Display for ScoredRunner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.play)
    }
}

#[derive(Copy, Clone)]
pub enum BoldingDisplayKind {
    None,
    Always,
    WinningTeam,
    MostRecentlyScored
}

impl BoldingDisplayKind {
    pub fn bolding(self, away_runs: usize, home_runs: usize, home_team_scored_most_recently: bool) -> (&'static str, &'static str) {
        const BOLD: &'static str = "**";
        const NONE: &'static str = "";

        match self {
            Self::None => (NONE, NONE),
            Self::Always => (BOLD, BOLD),
            Self::WinningTeam => match away_runs.cmp(&home_runs) {
                Ordering::Less => (NONE, BOLD),
                Ordering::Equal => (NONE, NONE),
                Ordering::Greater => (BOLD, NONE),
            }
            Self::MostRecentlyScored => if home_team_scored_most_recently { (NONE, BOLD) } else { (BOLD, NONE) },
        }
    }
}

#[derive(Clone)]
pub struct Score {
    away_abbreviation: String,
    pub away_runs: usize,
    home_abbreviation: String,
    pub home_runs: usize,
    innings: u8,
    pub home_team_scored_most_recently: bool,
    runs_bolding: BoldingDisplayKind,
    team_bolding: BoldingDisplayKind,
}

impl Score {
    pub fn new(away_abbreviation: String,
               away_runs: usize,
               home_abbreviation: String,
               home_runs: usize,
               innings: u8,
               home_team_scored_most_recently: bool,
               runs_bolding: BoldingDisplayKind, 
               team_bolding: BoldingDisplayKind) -> Self {
        Self {
            away_abbreviation,
            away_runs,
            home_abbreviation,
            home_runs,
            innings,
            home_team_scored_most_recently,
            runs_bolding,
            team_bolding,
        }
    }

    pub fn from_stats_log(home: &TeamStatsLog, away: &TeamStatsLog, innings: u8, home_team_scored_most_recently: bool, runs_bolding: BoldingDisplayKind, team_bolding: BoldingDisplayKind) -> Self {
        Self::new(away.abbreviation.clone(), away.runs, home.abbreviation.clone(), home.runs, innings, home_team_scored_most_recently, runs_bolding, team_bolding)
    }

    pub fn format_code_block(&self) -> String {
        let Self { away_abbreviation, away_runs, home_abbreviation, home_runs, home_team_scored_most_recently, .. } = self;
        let (home_bold, away_bold) = if *home_team_scored_most_recently { ("**", "") } else { ("", "**") };
        format!("{away_bold}`{away_abbreviation} {away_runs}`{away_bold} - {home_bold}`{home_abbreviation} {home_runs}`{home_bold}")
    }
}

impl Debug for Score {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { away_abbreviation, away_runs, home_abbreviation, home_runs, innings, home_team_scored_most_recently, runs_bolding, team_bolding } = self;
        let (away_abbreviation_bold, home_abbreviation_bold) = team_bolding.bolding(*away_runs, *home_runs, *home_team_scored_most_recently);
        let (away_score_bold, home_score_bold) = runs_bolding.bolding(*away_runs, *home_runs, *home_team_scored_most_recently);
        let extra_innings_suffix = if *innings > 9 { format!(" ({innings})") } else { String::new() };
        write!(f, "{away_abbreviation_bold}{away_abbreviation}{away_abbreviation_bold} {away_score_bold}{away_runs}{away_score_bold}-{home_score_bold}{home_runs}{home_score_bold} {home_abbreviation_bold}{home_abbreviation}{home_abbreviation_bold}{extra_innings_suffix}")
    }
}

impl Display for Score {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { away_abbreviation, away_runs, home_abbreviation, home_runs, .. } = self;
        write!(f, "{away_abbreviation} {away_runs}-{home_runs} {home_abbreviation}")
    }
}

pub fn pitching_stats(value: Value) -> Result<(f64, f64, char)> {
    let hand = value["people"][0]["pitchHand"]["code"].as_str().context("Could not get pitcher's hand")?.to_owned().chars().next().unwrap_or('R');
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

pub fn modify_abbreviation(parent: &Value) -> Result<String> {
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

pub fn get_last_lineup_underscores(previous_lineup: &Value) -> Result<[HitterLineupEntry; 9]> {
    let default_batting_order = vec![hide("Babe Ruth"), hide("Shohei Ohtani"), hide("Kevin Gausman"), hide("Barry Bonds"), hide("Ronald Acuña Jr."), hide("Mariano Rivera"), hide("Jacob deGrom"), hide("Ichiro Suzuki"), hide("Dave Stieb")];
    let players = &previous_lineup["players"];
    let vec = match previous_lineup["battingOrder"].as_array() {
        Some(iter) => iter.iter().filter_map(|id| id.as_i64()).filter_map(|x| players[&format!("ID{x}")]["person"]["fullName"].as_str()).map(hide).collect::<Vec<String>>(),
        None => default_batting_order.clone(),
    };
    let lineup = if vec.len() == 9 { vec } else { default_batting_order };
    let lineup = lineup.into_iter().enumerate().map(|(idx, name)| HitterLineupEntry::new(name, "__".to_owned(), idx as u8 + 1, None)).collect::<Vec<_>>();
    let [a, b, c, d, e, f, g, h, i] = lineup.as_slice() else { return Err(anyhow!("Batting order was not 9 batters in length")) };
    Ok([a.clone(), b.clone(), c.clone(), d.clone(), e.clone(), f.clone(), g.clone(), h.clone(), i.clone()])
}

pub fn lineup(root: &Value, first_stat: HittingStat, second_stat: HittingStat, show_stats: bool, team_name: &str) -> Result<[HitterLineupEntry; 9]> {
    let mut players = [const { MaybeUninit::uninit() }; 9];
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
        players[batting_order as usize / 100 - 1].write(HitterLineupEntry::new(name.to_owned(), position.to_owned(), (batting_order / 100) as u8, if show_stats { Some((first_stat, second_stat)) } else { None }));
    }
    Ok(unsafe { MaybeUninit::array_assume_init(players) })
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
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
            format!("{left} left field{right}")
        } else if let Some((left, right)) = event.split_once(" center fielder") {
            let Some(right) = remove_prefix(right.trim_start(), all_player_names.iter().map(String::as_str)) else { break; };
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
