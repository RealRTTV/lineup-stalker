use crate::components::hitting::HitterLineupEntry;
use crate::util::hide;
use crate::util::stat::HittingStat;
use anyhow::{bail, Context, Result};
use core::fmt::{Debug, Display, Formatter};
use fxhash::FxHashMap;
use mlb_api::game::{BattingOrderIndex, LiveFeedResponse, TeamWithGameData};
use mlb_api::meta::GameType;
use mlb_api::person::{Ballplayer, PersonId};
use mlb_api::season::SeasonId;
use mlb_api::team::TeamName;
use mlb_api::{single_stat, HomeAway, TeamSide};
use pollster::FutureExt;
use std::cmp::Ordering;
use mlb_api::stats::derived::era;
use crate::components::pitching::PitcherLineupEntry;

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
    
    pub fn from_description(description: &str, all_players: &FxHashMap<PersonId, Ballplayer<()>>) -> Vec<Self> {
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
            let value = if all_players.values().any(|player| value == player.full_name) && let Some(next) = iter.next() {
                remap_score_event(&format!("{value} {next}"), all_players)
            } else {
                remap_score_event(&value, all_players)
            };

            let scoring = value.contains("scores.") || value.contains("homers") || value.contains("home run") || value.contains("grand slam");
            vec.push(ScoredRunner::new(value, scoring));
        }
        vec
    }
    
    pub fn as_one_liner(&self) -> OneLiner<'_> {
        OneLiner(self)
    }
}

#[must_use]
pub struct OneLiner<'a>(&'a ScoredRunner);

impl Display for ScoredRunner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.scoring {
            write!(f, "> **{}**", self.play)
        } else {
            write!(f, "> {}", self.play)
        }
    }
}

impl Display for OneLiner<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.play)
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
    pub fn bolding(self, away_runs: usize, home_runs: usize, who_scored: TeamSide) -> (&'static str, &'static str) {
        const BOLD: &str = "**";
        const NONE: &str = "";

        match self {
            Self::None => (NONE, NONE),
            Self::Always => (BOLD, BOLD),
            Self::WinningTeam => match away_runs.cmp(&home_runs) {
                Ordering::Less => (NONE, BOLD),
                Ordering::Equal => (NONE, NONE),
                Ordering::Greater => (BOLD, NONE),
            }
            Self::MostRecentlyScored => if home_runs == away_runs { (NONE, NONE) } else { HomeAway::new((NONE, BOLD), (BOLD, NONE)).choose(who_scored) },
        }
    }
}

#[derive(Clone)]
pub struct Score {
    pub away_abbreviation: String,
    pub away_runs: usize,
    pub home_abbreviation: String,
    pub home_runs: usize,
    pub innings: u8,
    pub who_scored: TeamSide,
    pub runs_bolding: BoldingDisplayKind,
    pub team_bolding: BoldingDisplayKind,
}

impl Score {
    pub fn new(away_abbreviation: String,
               away_runs: usize,
               home_abbreviation: String,
               home_runs: usize,
               innings: u8,
               who_scored: TeamSide,
               runs_bolding: BoldingDisplayKind, 
               team_bolding: BoldingDisplayKind) -> Self {
        Self {
            away_abbreviation,
            away_runs,
            home_abbreviation,
            home_runs,
            innings,
            who_scored,
            runs_bolding,
            team_bolding,
        }
    }

    pub fn as_code_block(&self) -> CodeBlock<'_> {
        CodeBlock(self)
    }
    
    pub fn one_liner(&self) -> OneLinerScore<'_> {
        OneLinerScore(self)
    }
}

impl Display for Score {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self { away_abbreviation, away_runs, home_abbreviation, home_runs, innings, who_scored, runs_bolding, team_bolding } = self;
        let (away_abbreviation_bold, home_abbreviation_bold) = team_bolding.bolding(*away_runs, *home_runs, *who_scored);
        let (away_score_bold, home_score_bold) = runs_bolding.bolding(*away_runs, *home_runs, *who_scored);
        let extra_innings_suffix = if *innings > 9 { format!(" ({innings})") } else { String::new() };
        write!(f, "{away_abbreviation_bold}{away_abbreviation}{away_abbreviation_bold} {away_score_bold}{away_runs}{away_score_bold}-{home_score_bold}{home_runs}{home_score_bold} {home_abbreviation_bold}{home_abbreviation}{home_abbreviation_bold}{extra_innings_suffix}")
    }
}

#[must_use]
pub struct CodeBlock<'a>(&'a Score);

impl Display for CodeBlock<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Score { away_abbreviation, away_runs, home_abbreviation, home_runs, who_scored, .. } = self.0;
        let (home_bold, away_bold) = HomeAway::new(("**", ""), ("", "**")).choose(*who_scored);
        write!(f, "{away_bold}`{away_abbreviation} {away_runs}`{away_bold} - {home_bold}`{home_abbreviation} {home_runs}`{home_bold}")
    }
}

#[must_use]
pub struct OneLinerScore<'a>(&'a Score);

impl Display for OneLinerScore<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Score { away_abbreviation, away_runs, home_abbreviation, home_runs, .. } = self.0;
        write!(f, "{away_abbreviation} {away_runs}-{home_runs} {home_abbreviation}")
    }
}

// todo: use more
pub fn stalker_abbreviation(name: &TeamName) -> String {
    if name.abbreviation.len() == 3 {
        return name.abbreviation.clone();
    }
    let acronym = name.franchise_name
        .split(' ')
        .chain(name.club_name.split(' '))
        .filter_map(|s| s.chars().next())
        .collect::<String>();
    if acronym.len() == 3 {
        return acronym;
    }
    if name.team_code.len() == 3 {
        return name.team_code.to_ascii_uppercase();
    }
    acronym
}

pub fn get_last_lineup_underscores(team: Option<TeamWithGameData>) -> [HitterLineupEntry; 9] {
    let mut idx = 1;

    if let Some(team) = team && let Some(batting_order) = team.batting_order {
        let players = &team.players;
        batting_order.map(|id| hide(&players[&id].person.full_name))
    } else {
        [
            hide("____ ____"),
            hide("______ ______"),
            hide("_____ _______"),
            hide("_____ _____"),
            hide("______ _____ ___"),
            hide("_______ ______"),
            hide("_____ ______"),
            hide("______ ______"),
            hide("____ _____")
        ]
    }.map(|name| {
        let entry = HitterLineupEntry::new(name, None, BattingOrderIndex { major: idx, minor: 0 }, None);
        idx += 1;
        entry
    })
}

pub fn lineup(team: &TeamWithGameData, stats: [HittingStat; 2], show_stats: bool, season: SeasonId) -> Result<[HitterLineupEntry; 9]> {
    let mut idx = 0;
    Ok(team.batting_order.context("Hitter was missing from lineup")?
        .map(|id| &team.players[&id])
        .map(|player| {
            let person = &player.person;
            let name = &person.full_name;
            let position = player.position.clone();
            let sabermetrics_stats = {
                let id = person.id;
                async move || single_stat!(Sabermetrics + Hitting for id; with |builder| builder.season(season)).await
            };
            let stats = if show_stats { Some(stats.map(|stat| stat.get(&player.season_stats.hitting, sabermetrics_stats)).map(|future| future.block_on())) } else { None };
            let entry = HitterLineupEntry::new(name.to_owned(), Some(position), BattingOrderIndex { major: idx + 1, minor: 0 }, stats);
            idx += 1;
            entry
        }))
}

pub fn should_show_stats(game_type: GameType) -> bool {
    matches!(game_type, GameType::RegularSeason) || game_type.is_postseason()
}

pub fn remap_score_event(event: &str, all_players: &FxHashMap<PersonId, Ballplayer<()>>) -> String {
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
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} left field{right}")
        } else if let Some((left, right)) = event.split_once(" center fielder") {
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} center field{right}")
        } else if let Some((left, right)) = event.split_once(" right fielder") {
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} right field{right}")
        } else if let Some((left, right)) = event.split_once(" first baseman") {
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} first base{right}")
        } else if let Some((left, right)) = event.split_once(" second baseman") {
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} second base{right}")
        } else if let Some((left, right)) = event.split_once(" third baseman") {
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} third base{right}")
        } else if let Some((left, right)) = event.split_once(" catcher") {
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} catcher{right}")
        } else if let Some((left, right)) = event.split_once(" pitcher") {
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} pitcher{right}")
        } else if let Some((left, right)) = event.split_once(" shortstop") {
            let Some(right) = remove_prefix(right.trim_start(), all_players.values().map(|player| player.full_name.as_str())) else { break; };
            format!("{left} shortstop{right}")
        } else {
            break;
        }
    }

    event.replace("1st", "first").replace("2nd", "second").replace("3rd", "third")
}

pub fn probable_pitchers(live_feed: &LiveFeedResponse) -> Result<HomeAway<PitcherLineupEntry>> {
    let HomeAway { home: Some(home), away: Some(away) } = live_feed.data.probable_pitchers.as_ref() else { bail!("Expected probable pitchers") };

    Ok(HomeAway::new(home, away).zip(live_feed.data.teams.as_ref().map(|team| stalker_abbreviation(&team.name))).zip(live_feed.live.boxscore.teams.as_ref()).map(|((person, abbreviation), team)| {
        let pitcher = &team.players[&person.id];
        let person = &live_feed.data.players[&person.id];
        PitcherLineupEntry::new(person.full_name.clone(), person.id, abbreviation, person.pitch_hand, era(pitcher.season_stats.pitching.earned_runs, pitcher.season_stats.pitching.innings_pitched), pitcher.season_stats.pitching.innings_pitched.unwrap_or_default())
    }))
}
