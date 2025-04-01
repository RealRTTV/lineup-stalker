#![feature(maybe_uninit_array_assume_init)]
#![feature(result_flattening)]
#![feature(let_chains)]

use core::ffi::c_void;
use core::str::FromStr;
use std::io::{stderr, stdout};
use std::convert::identity;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{Datelike, DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;
use chrono_tz::Tz::America__Toronto;
use fxhash::{FxHashMap, FxHashSet};
use serde_json::Value;

use crate::posts::defensive_substitution::DefensiveSubstitution;
use crate::posts::defensive_switch::DefensiveSwitch;
use crate::posts::final_card::FinalCard;
use crate::posts::lineup::Lineup;
use crate::posts::offensive_substitution::OffensiveSubstitution;
use crate::posts::pitching_substitution::PitchingSubstitution;
use crate::posts::Post;
use crate::posts::scoring_play::ScoringPlay;
use crate::posts::scoring_play_event::ScoringPlayEvent;
use crate::util::{clear_screen, get_team_color_escape, statsapi};
use crate::util::decisions::Decisions;
use crate::util::fangraphs::{BALLPARK_ADJUSTMENTS, WOBA_CONSTANTS};
use crate::util::ffi::{_getch, ConsoleCursorInfo, Coordinate, GetStdHandle, SetConsoleCursorInfo, SetConsoleCursorPosition, SetConsoleTextAttribute};
use crate::util::line_score::LineScore;
use crate::util::next_game::NextGame;
use crate::util::pitching::PitcherLineupEntry;
use crate::util::record_against::RecordAgainst;
use crate::util::standings::Standings;
use crate::util::stat::HittingStat;
use crate::util::statsapi::{pitching_stats, modify_abbreviation, get_last_lineup_underscores, Score, BoldingDisplayKind};
use crate::util::team_stats_log::TeamStatsLog;

pub const TIMEZONE: Tz = America__Toronto;

pub mod util;
pub mod posts;

fn main() {
    let _ = WOBA_CONSTANTS.deref();
    let _ = BALLPARK_ADJUSTMENTS.deref();

    loop {
        clear_screen(128);
        set_cursor(0, 0);
        if let Err(e) = unsafe { main0() } {
            eprintln!("Error while stalking lineup: {e}");
        }
        eprint!("\nPress any key to continue... ");
        let _ = std::io::Write::flush(&mut stderr());
        unsafe { _getch() };
    }
}

pub fn get(url: &str) -> Result<Value> {
    get_with_sleep(url, Duration::from_millis(2500))
}

pub fn get_with_sleep(url: &str, duration: Duration) -> Result<Value> {
    loop {
        return match ureq::get(url).call() {
            Ok(response) => response,
            Err(_) => {
                std::thread::sleep(duration);
                continue;
            }
        }.into_json::<Value>().context("Response was not a valid json")
    }
}

fn set_cursor(x: usize, y: usize) {
    unsafe { SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: x as i16, y: y as i16 }); }
}

unsafe fn main0() -> Result<()> {
    SetConsoleCursorInfo(
        GetStdHandle(-11_i32 as u32),
        &ConsoleCursorInfo::new(1, false),
    );
    let (id, home, first_stat, second_stat) = get_id()?;
    let url = format!("https://statsapi.mlb.com/api/v1.1/game/{id}/feed/live");
    set_cursor(0, 0);
    let mut response = get(&url)?;
    let game_id = response["gameData"]["game"]["pk"]
        .as_i64()
        .context("Game ID didn't exist")?;
    let (lineup, next_game, (home_pitcher_id, away_pitcher_id)) = lines(&response, home, game_id, first_stat, second_stat)?;
    let mut post = Post::Lineup(lineup);
    post.send_with_settings(true, true, true)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut dots = 0;
        SetConsoleCursorInfo(
            GetStdHandle(-11_i32 as u32),
            &ConsoleCursorInfo::new(1, false),
        );

        let cancelled_clone = Arc::clone(&cancelled);
        std::thread::spawn(move || {
            loop {
                let key = unsafe { _getch() };
                if key == 0x08 {
                    cancelled_clone.store(true, Ordering::Relaxed);
                    break;
                }
            }
        });

        loop {
            if cancelled.load(Ordering::Relaxed) {
                return Ok(())
            }

            if response["liveData"]["boxscore"]["teams"][if home { "home" } else { "away" }]
                ["battingOrder"]
                .as_array()
                .map_or(true, Vec::is_empty)
            {
                print!("\rLoading{: <pad$}", ".".repeat(dots + 1), pad = 3 - dots);
                std::io::Write::flush(&mut stdout())?;
                dots = (dots + 1) % 3;
                response = get(&url)?;
                std::thread::sleep(Duration::new(1, 0));
            } else {
                println!("         ");
                break;
            }
        }
        SetConsoleCursorInfo(
            GetStdHandle(-11_i32 as u32),
            &ConsoleCursorInfo::new(1, true),
        );
    }
    set_cursor(0, 0);
    if let Post::Lineup(inner) = &mut post {
        let lineup = statsapi::lineup(&response["liveData"]["boxscore"]["teams"][if home { "home" } else { "away" }], first_stat, second_stat, response["gameData"]["game"]["type"].as_str().context("Expected game type")? != "R" || response["gameData"]["game"]["type"].as_str().context("Expected game type")? != "S", &response["gameData"]["teams"][if home { "home" } else { "away" }]["teamName"].as_str().context("Expected team name")?)?;
        inner.update_lineup(lineup);
        post.send()?;
    }
    let Post::Lineup(Lineup { record, standings, .. }) = post else { return Err(anyhow!("Post was not a lineup??")) };
    posts_loop(
        cancelled,
        response,
        home,
        standings,
        record,
        next_game,
        home_pitcher_id,
        away_pitcher_id,
    )?;
    Ok(())
}

fn get_id() -> Result<(usize, bool, HittingStat, HittingStat)> {
    const PREFERRED_TEAMS: &[&str] = &[
        "Toronto Blue Jays"
    ];

    fn print_game(game: &Value, idx: usize, handle: *mut c_void, idx_width: usize, default_color_escape: &str, preferred_team_color_escape: &str, always_use_default_color: bool) -> Result<()> {
        let idx = idx + 1;
        let home = game["teams"]["home"]["team"]["name"]
            .as_str()
            .context("Home Team name didn't exist")?;
        let away = game["teams"]["away"]["team"]["name"]
            .as_str()
            .context("Away Team name didn't exist")?;
        let (color_escape, home_color_escape, away_color_escape) = if PREFERRED_TEAMS.contains(&home) || PREFERRED_TEAMS.contains(&away) {
            (preferred_team_color_escape, preferred_team_color_escape, preferred_team_color_escape)
        } else {
            (default_color_escape, get_team_color_escape(home), get_team_color_escape(away))
        };
        let time = chrono::DateTime::<Local>::from_str(
            game["gameDate"]
                .as_str()
                .context("Game Date didn't exist")?,
        )?;
        let timestamp = TIMEZONE
            .from_local_datetime(&time.naive_local())
            .latest()
            .context("Error converting to timezone")?
            .format("%H:%M %Z");
        if always_use_default_color {
            println!("\x1B[{color_escape}m  {idx: >idx_width$}. {home} vs. {away} @ {timestamp}\x1B[{color_escape}m");
        } else {
            println!("\x1B[{color_escape}m  {idx: >idx_width$}. \x1B[{home_color_escape}m{home}\x1B[{color_escape}m vs. \x1B[{away_color_escape}m{away}\x1B[{color_escape}m @ {timestamp}");
        }
        unsafe {
            SetConsoleTextAttribute(handle, 7);
        }
        Ok(())
    }

    let mut idx = 0_usize;
    let mut date = Local::now().date_naive();
    let handle = unsafe { GetStdHandle(-11_i32 as u32) };
    'a: loop {
        set_cursor(0, 0);
        let response = get(&format!(
            "https://statsapi.mlb.com/api/v1/schedule/games/?sportId=1&date={}",
            date.format("%m/%d/%Y")
        ))?;
        let games = response["dates"][0]["games"]
            .as_array()
            .unwrap_or(const { &vec![] });
        let mut ids = Vec::with_capacity(games.len());
        let idx_width = (games.len() + 1).checked_ilog10().map_or(1, |x| x + 1) as usize;
        println!("[{}] Please select a game ordinal to wait on for lineups (use arrows for movement and dates): \n", date.format("%A, %B %e %Y"));
        for (idx, game) in games.iter().enumerate() {
            ids.push(game["gamePk"].as_i64().context("Game ID didn't exist")?);
            print_game(game, idx, handle, idx_width, "0", "38;5;10", false)?;
        }
        set_cursor(0, 2);
        print!("> ");
        std::io::Write::flush(&mut stdout())?;
        set_cursor(0, 2);
        loop {
            let first = unsafe { _getch() };
            if first == 0x33 {
                idx = 0;
                date = date.with_day(1).context("Error whens etting day to 1")?.with_month(3).context("Error when setting month to march")?;
                clear_screen(ids.len() + 2);
                set_cursor(0, 0);
                continue 'a;
            } else if first == 0x34 {
                idx = 0;
                date = date.with_day(1).context("Error whens etting day to 1")?.with_month(4).context("Error when setting month to april")?;
                clear_screen(ids.len() + 2);
                set_cursor(0, 0);
                continue 'a;
            } else if first == 0x35 {
                idx = 0;
                date = date.with_day(1).context("Error whens etting day to 1")?.with_month(5).context("Error when setting month to may")?;
                clear_screen(ids.len() + 2);
                set_cursor(0, 0);
                continue 'a;
            } else if first == 0x36 {
                idx = 0;
                date = date.with_day(1).context("Error whens etting day to 1")?.with_month(6).context("Error when setting month to june")?;
                clear_screen(ids.len() + 2);
                set_cursor(0, 0);
                continue 'a;
            } else if first == 0x37 {
                idx = 0;
                date = date.with_day(1).context("Error whens etting day to 1")?.with_month(7).context("Error when setting month to july")?;
                clear_screen(ids.len() + 2);
                set_cursor(0, 0);
                continue 'a;
            } else if first == 0x38 {
                idx = 0;
                date = date.with_day(1).context("Error whens etting day to 1")?.with_month(8).context("Error when setting month to august")?;
                clear_screen(ids.len() + 2);
                set_cursor(0, 0);
                continue 'a;
            } else if first == 0x39 {
                idx = 0;
                date = date.with_month(9).context("Error when setting month to september")?;
                clear_screen(ids.len() + 2);
                set_cursor(0, 0);
                continue 'a;
            } else if first == 0x30 {
                idx = 0;
                date = date.with_day(1).context("Error whens etting day to 1")?.with_month(10).context("Error when setting month to october")?;
                clear_screen(ids.len() + 2);
                set_cursor(0, 0);
                continue 'a;
            } else if first == 0xE0 {
                let second = unsafe { _getch() };
                if second == 0x48 {
                    set_cursor(0, idx + 2);
                    print!("  ");
                    std::io::Write::flush(&mut stdout())?;
                    idx = idx.saturating_sub(1);
                    set_cursor(0, idx + 2);
                    print!("> ");
                    std::io::Write::flush(&mut stdout())?;
                } else if second == 0x50 {
                    set_cursor(0, idx + 2);
                    print!("  ");
                    std::io::Write::flush(&mut stdout())?;
                    idx = (idx + 1).min(ids.len() - 1);
                    set_cursor(0, idx + 2);
                    print!("> ");
                    std::io::Write::flush(&mut stdout())?;
                } else if second == 0x4B {
                    idx = 0;
                    loop {
                        date = date
                            .pred_opt()
                            .context("Error when getting previous date")?;
                        let response = get(&format!(
                            "https://statsapi.mlb.com/api/v1/schedule/games/?sportId=1&date={}",
                            date.format("%m/%d/%Y")
                        ))?;
                        if response["dates"][0]["games"].as_array().map_or(true, |list| list.is_empty()) {
                            continue
                        } else {
                            break
                        }
                    }
                    clear_screen(ids.len() + 2);
                    set_cursor(0, 0);
                    continue 'a;
                } else if second == 0x4D {
                    idx = 0;
                    loop {
                        date = date
                            .succ_opt()
                            .context("Error when getting next date")?;
                        let response = get(&format!(
                            "https://statsapi.mlb.com/api/v1/schedule/games/?sportId=1&date={}",
                            date.format("%m/%d/%Y")
                        ))?;
                        if response["dates"][0]["games"].as_array().map_or(true, |list| list.is_empty()) {
                            continue
                        } else {
                            break
                        }
                    }
                    clear_screen(ids.len() + 2);
                    set_cursor(0, 0);
                    continue 'a;
                } else {
                    println!("{second:x}");
                    loop {}
                }
            } else if first == 0x0D && !games.is_empty() {
                set_cursor(0, 2);
                for (current_idx, game) in games.iter().enumerate() {
                    if current_idx == idx {
                        print_game(game, current_idx, handle, idx_width, "0", "38;5;10", false)?;
                    } else {
                        print_game(game, current_idx, handle, idx_width, "90", "38;5;9", true)?;
                    }
                    std::thread::sleep(Duration::from_millis(35 - current_idx as u64));
                }
                set_cursor(0, 0);
                println!("[{}] Please select the home team or away team (use arrows for switching):                                \n", date.format("%A, %B %e %Y"));
                set_cursor(0, idx + 2);
                let game = &games[idx];
                let home = game["teams"]["home"]["team"]["name"]
                    .as_str()
                    .context("Home Team name didn't exist")?;
                let away = game["teams"]["away"]["team"]["name"]
                    .as_str()
                    .context("Away Team name didn't exist")?;
                let time = chrono::DateTime::<Local>::from_str(
                    game["gameDate"]
                        .as_str()
                        .context("Game Date didn't exist")?,
                )?;
                unsafe { SetConsoleTextAttribute(handle, 7); }
                println!(
                    "> \x1B[{home_color_escape}m{home}\x1B[0m vs. \x1B[{away_color_escape}m{away}\x1B[0m @ {timestamp}                                ",
                    home_color_escape = get_team_color_escape(home),
                    away_color_escape = get_team_color_escape(away),
                    timestamp = TIMEZONE
                        .from_local_datetime(&time.naive_local())
                        .latest()
                        .context("Error converting to timezone")?
                        .format("%H:%M %Z")
                );
                print!("  {home_underline}                                                                \r", home_underline = "^".repeat(home.len()));
                std::io::Write::flush(&mut stdout())?;
                let mut is_home = true;
                loop {
                    let first = unsafe { _getch() };
                    if first == 0xE0 {
                        let second = unsafe { _getch() };
                        if second == 0x4B || second == 0x4D {
                            is_home = !is_home;
                            let (home_symbol, away_symbol) = if is_home { ("^", " ") } else { (" ", "^") };
                            print!("  {home_underline}     {away_underline}                                                                \r", home_underline = home_symbol.repeat(home.len()), away_underline = away_symbol.repeat(away.len()));
                            std::io::Write::flush(&mut stdout())?;
                        }
                    } else if first == 0x0D {
                        clear_screen(ids.len() + 2);
                        set_cursor(0, 0);
                        println!("[{}] Please select hitting stats (use arrows):                                \n", date.format("%A, %B %e %Y"));
                        let mut stats = [HittingStat::AVG, HittingStat::wRCp];
                        let mut selected_stat_idx = 0_usize;
                        loop {
                            set_cursor(0, 2);
                            {
                                unsafe { SetConsoleTextAttribute(handle, 8); }
                                print!("  ");
                                for (idx, stat) in stats.iter().enumerate() {
                                    print!("{prev: ^width$}", prev = stat.prev().to_string(), width = HittingStat::MAX_NAME_WIDTH);
                                    if idx + 1 < stats.len() {
                                        print!(" | ");
                                    }
                                }
                                println!();

                            }
                            {
                                unsafe { SetConsoleTextAttribute(handle, 7); }
                                print!("{arrow} ", arrow = if selected_stat_idx == 0 { '>' } else { ' ' });
                                for (idx, stat) in stats.iter().enumerate() {
                                    print!("{stat: ^width$}", stat = stat.to_string(), width = HittingStat::MAX_NAME_WIDTH);
                                    if idx + 1 < stats.len() {
                                        print!(" | ");
                                    }
                                }
                                print!(" {arrow}", arrow = if selected_stat_idx == 1 { '<' } else { ' ' });
                                println!();
                            }
                            {
                                unsafe { SetConsoleTextAttribute(handle, 8); }
                                print!("  ");
                                for (idx, stat) in stats.iter().enumerate() {
                                    print!("{next: ^width$}", next = stat.next().to_string(), width = HittingStat::MAX_NAME_WIDTH);
                                    if idx + 1 < stats.len() {
                                        print!(" | ");
                                    }
                                }
                                println!();
                                unsafe { SetConsoleTextAttribute(handle, 7); }
                            }
                            let first = unsafe { _getch() };
                            if first == 0xE0 {
                                let second = unsafe { _getch() };
                                if second == 0x4B || second == 0x4D {
                                    selected_stat_idx = 1 - selected_stat_idx;
                                } else if second == 0x48 {
                                    stats[selected_stat_idx] = stats[selected_stat_idx].prev();
                                } else if second == 0x50 {
                                    stats[selected_stat_idx] = stats[selected_stat_idx].next();
                                }
                            } else if first == 0x0D {
                                clear_screen(5);
                                return Ok((ids[idx] as usize, is_home, stats[0], stats[1]))
                            }
                        }
                    }
                }
            }
        }
    }
}

unsafe fn posts_loop(
    cancelled: Arc<AtomicBool>,
    response: Value,
    is_home_team: bool,
    mut standings: Standings,
    mut record: RecordAgainst,
    next_game: Option<NextGame>,
    mut home_pitcher_id: i64,
    mut away_pitcher_id: i64,
) -> Result<()> {
    let game_id = response["gamePk"].as_i64().context("Could not get game id")?;
    let id_to_object = response["gameData"]["players"]
        .as_object()
        .context("Could not find home players list")?
        .values()
        .filter_map(|player| player["id"].as_i64().map(|id| (id, player.clone())))
        .collect::<FxHashMap<_, _>>();
    let all_player_names = id_to_object
        .values()
        .filter_map(|player| player["fullName"].as_str().map(ToOwned::to_owned))
        .collect::<Vec<String>>();
    let mut scoring_plays = Vec::new();
    let mut previous_play_plus_play_event_len = 0;

    let mut home = TeamStatsLog::new(id_to_object.get(&home_pitcher_id).context("Pitcher's name should exist")?["lastName"].as_str().context("Expected pitcher's name")?.to_owned(), modify_abbreviation(&response["gameData"]["teams"]["home"])?);
    let mut away = TeamStatsLog::new(id_to_object.get(&away_pitcher_id).context("Pitcher's name should exist")?["lastName"].as_str().context("Expected pitcher's name")?.to_owned(), modify_abbreviation(&response["gameData"]["teams"]["away"])?);

    let mut first_time_around = true;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(())
        }
        if !core::mem::replace(&mut first_time_around, false) {
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::new(2, 0) {
                if cancelled.load(Ordering::Relaxed) {
                    return Ok(())
                }
            }
        }
        let pbp = get(&format!("https://statsapi.mlb.com/api/v1/game/{game_id}/playByPlay"))?;
        let all_plays = pbp["allPlays"].as_array().context("Game must have a list of plays")?;
        for (is_top_inning, parent, play) in all_plays
            .iter()
            .map(|play| (play["about"]["isTopInning"].as_bool().unwrap(), play))
            .flat_map(|(away, parent)| {
                parent["playEvents"].as_array().unwrap_or(const { &vec![] }).iter()
                    .chain(std::iter::once(parent))
                    .map(move |play| (away, parent, play))
            })
            .skip(previous_play_plus_play_event_len)
        {
            if play["type"].is_null() {
                if !play["about"]["isComplete"]
                    .as_bool()
                    .unwrap()
                {
                    break;
                };
                let desc = play["result"]["description"]
                    .as_str()
                    .unwrap();
                if let Some("walk" | "intent_walk") = play["result"]["eventType"].as_str() {
                    if is_top_inning {
                        away.walk();
                    } else {
                        home.walk();
                    }
                }
                if play["result"]["eventType"].as_str() == Some("strikeout") {
                    if is_top_inning {
                        away.strikeout();
                    } else {
                        home.strikeout();
                    }
                }
                if !(desc.contains("home run") || desc.contains("homers") || desc.contains("scores")) {
                    continue;
                };
                let scoring_play = ScoringPlay::from_play(
                    play,
                    &home.abbreviation,
                    &away.abbreviation,
                    &all_player_names,
                )?;
                scoring_plays.push(scoring_play.one_liner());
                Post::ScoringPlay(scoring_play).send()?;
            } else {
                if play["type"].as_str().unwrap() == "pitch" {
                    if is_top_inning {
                        home.pitch_thrown();
                    } else {
                        away.pitch_thrown();
                    }
                } else if play["type"].as_str().unwrap() == "action" {
                    match play["details"]["eventType"]
                        .as_str()
                        .unwrap()
                    {
                        "pitching_substitution" => {
                            let previous_pitcher_id = if is_top_inning {
                                home_pitcher_id
                            } else {
                                away_pitcher_id
                            };
                            let pitching_substitution = PitchingSubstitution::from_play(
                                play,
                                if is_top_inning { &home.abbreviation } else { &away.abbreviation },
                                get(&format!("https://statsapi.mlb.com/api/v1/people/{previous_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?
                            )?;
                            if is_top_inning {
                                home_pitcher_id = pitching_substitution.new_id();
                                home.change_pitcher(pitching_substitution.old_last_name().to_owned());
                            } else {
                                away_pitcher_id = pitching_substitution.new_id();
                                away.change_pitcher(pitching_substitution.old_last_name().to_owned());
                            }
                            Post::PitchingSubstitution(pitching_substitution).send()?;
                        }
                        "offensive_substitution" => {
                            let offensive_substitution = OffensiveSubstitution::from_play(
                                play,
                                parent,
                                if is_top_inning {
                                    &away.abbreviation
                                } else {
                                    &home.abbreviation
                                },
                                &id_to_object,
                            )?;
                            Post::OffensiveSubstitution(offensive_substitution).send()?;
                        }
                        "defensive_substitution" => {
                            let defensive_substitution = DefensiveSubstitution::from_play(
                                play,
                                parent,
                                if is_top_inning {
                                    &home.abbreviation
                                } else {
                                    &away.abbreviation
                                },
                                &id_to_object,
                            )?;
                            Post::DefensiveSubstitution(defensive_substitution).send()?;
                        }
                        "defensive_switch" => {
                            let defensive_switch = DefensiveSwitch::from_play(
                                play,
                                parent,
                                if is_top_inning {
                                    &home.abbreviation
                                } else {
                                    &away.abbreviation
                                },
                                &id_to_object,
                            )?;
                            Post::DefensiveSwitch(defensive_switch).send()?;
                        }
                        "passed_ball" | "wild_pitch"
                            if play["details"]["isScoringPlay"]
                                .as_bool()
                                .context("Could not find if something was a scoring play")? =>
                        {
                            let passed_ball = ScoringPlayEvent::from_play(
                                play,
                                parent,
                                &home.abbreviation,
                                &away.abbreviation,
                                &all_player_names,
                                "Wild pitch",
                            )?;
                            scoring_plays.push(passed_ball.one_liner());
                            Post::PassedBall(passed_ball).send()?;
                        }
                        "stolen_base_home" => {
                            let stolen_home = ScoringPlayEvent::from_play(
                                play,
                                parent,
                                &home.abbreviation,
                                &away.abbreviation,
                                &all_player_names,
                                "Stolen base",
                            )?;
                            scoring_plays.push(stolen_home.one_liner());
                            Post::StolenHome(stolen_home).send()?;
                        }
                        _ => {}
                    }
                }
            }
        }

        previous_play_plus_play_event_len = all_plays.iter().map(|play| play["playEvents"].as_array().map_or(0, |vec| vec.len()) + play["about"]["isComplete"].as_bool().is_some_and(identity) as usize).sum();

        let mut response = get(&format!("https://statsapi.mlb.com/api/v1.1/game/{game_id}/feed/live"))?;
        if !response["liveData"]["decisions"]["winner"].is_null() {
            let linescore = &response["liveData"]["linescore"];
            let top = linescore["isTopInning"]
                .as_bool()
                .context("Could not find out if it's the top of the inning")?;
            let innings = linescore["innings"]
                .as_array()
                .context("Could not get innings")?;
            
            let num_innings = innings.len();

            for inning in innings {
                home.add_runs(inning["home"]["runs"].as_i64().unwrap_or(0) as usize);
                home.add_hits(inning["home"]["hits"].as_i64().unwrap_or(0) as usize);
                home.add_errors(inning["home"]["errors"].as_i64().unwrap_or(0) as usize);
                away.add_runs(inning["away"]["runs"].as_i64().unwrap_or(0) as usize);
                away.add_hits(inning["away"]["hits"].as_i64().unwrap_or(0) as usize);
                away.add_errors(inning["away"]["errors"].as_i64().unwrap_or(0) as usize);
            }

            let walkoff = home.runs > away.runs
                && home.runs
                - innings.last().context("You gotta have at least one inning if the game is over")?["home"]["runs"].as_i64().unwrap_or(0) as usize <= away.runs;
            let line_score = LineScore::new(innings, &home, &away, top)?;

            if (away.runs > home.runs) ^ is_home_team {
                standings.win();
                record.win();
            } else {
                standings.loss();
                record.loss();
            }

            let pitching_masterpiece = TeamStatsLog::generate_masterpiece(&home, &away, innings.len(), &home.abbreviation).unwrap_or(String::new()) + &TeamStatsLog::generate_masterpiece(&away, &home, innings.len(), &away.abbreviation).unwrap_or(String::new());
            let decisions = loop {
                match Decisions::new(&response) {
                    Ok(decisions) => {
                        response = get(&format!("https://statsapi.mlb.com/api/v1.1/game/{game_id}/feed/live"))?;
                        break Some(decisions)
                    },
                    Err(e) => {
                        eprintln!("Error getting pitcher decisions: {e}");
                        std::thread::sleep(Duration::from_secs(5));
                    },
                }
            };

            Post::FinalCard(FinalCard::new(Score::from_stats_log(&home, &away, num_innings as u8, false, BoldingDisplayKind::WinningTeam, if walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None }), standings, record, next_game, pitching_masterpiece, line_score, scoring_plays, decisions)).send()?;

            while !cancelled.load(Ordering::Relaxed) { core::hint::spin_loop() }
            return Ok(())
        }
    }
}

fn lines(
    response: &Value,
    home: bool,
    game_id: i64,
    first_hitting_stat: HittingStat,
    second_hitting_stat: HittingStat,
) -> Result<(Lineup, Option<NextGame>, (i64, i64))> {
    let home_full = response["gameData"]["teams"]["home"]["name"]
        .as_str()
        .context("Home Team didn't have a full name")?;
    let away_full = response["gameData"]["teams"]["away"]["name"]
        .as_str()
        .context("Away Team didn't have a full name")?;

    let (home_abbreviation, away_abbreviation) = (
        modify_abbreviation(&response["gameData"]["teams"]["home"])?,
        modify_abbreviation(&response["gameData"]["teams"]["away"])?,
    );

    let utc = DateTime::<Utc>::from_str(response["gameData"]["datetime"]["dateTime"].as_str().context("Game Date Time didn't exist")?)?.naive_utc();
    let datetime = TIMEZONE.from_utc_datetime(&utc);
    let local_datetime = Tz::from_str(response["gameData"]["venue"]["timeZone"]["id"].as_str().context("Could not find venue's local time zone for game")?).map_err(|e| anyhow!("{e}"))?.from_utc_datetime(&utc);
    let time = if datetime.naive_local() == local_datetime.naive_local() {
        format!("{}", datetime.format("%H:%M %Z"))
    } else {
        format!("{} / {}", datetime.format("%H:%M %Z"), local_datetime.format("%H:%M %Z"))
    };

    std::thread::scope(|s| {
        let pitcher_future = s.spawn(|| get_pitcher_lines(&response, &home_abbreviation, &away_abbreviation));

        let (previous_game, standings, record, next_game) = response_parsed_values(&response, home, game_id)?;

        let (previous, previous_team_lineup) = if let Some(previous_game) = previous_game {
            let home_runs = previous_game["liveData"]["boxscore"]["teams"]["home"]["teamStats"]["batting"]["runs"]
                .as_i64()
                .context("Home Team didn't have runs")? as usize;
            let away_runs = previous_game["liveData"]["boxscore"]["teams"]["away"]["teamStats"]["batting"]["runs"]
                .as_i64()
                .context("Away Team didn't have runs")? as usize;

            let (previous_home_abbreviation, previous_away_abbreviation) = (
                modify_abbreviation(&previous_game["gameData"]["teams"]["home"])?,
                modify_abbreviation(&previous_game["gameData"]["teams"]["away"])?,
            );

            let previous_innings = previous_game["liveData"]["linescore"]["innings"]
                .as_array()
                .context("Could not get innings")?
                .len();

            let walkoff = previous_innings >= 9 && home_runs > away_runs;

            let previous_team_lineup = previous_game["liveData"]["boxscore"]["teams"][if home {
                if previous_home_abbreviation == home_abbreviation {
                    "home"
                } else {
                    "away"
                }
            } else {
                if previous_away_abbreviation == away_abbreviation {
                    "away"
                } else {
                    "home"
                }
            }].clone();
            (Some(Score::new(previous_away_abbreviation, away_runs, previous_home_abbreviation, home_runs, previous_innings as u8, false, BoldingDisplayKind::WinningTeam, if walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None })), previous_team_lineup)
        } else {
            (None, Value::Null)
        };

        let title = if home {
            format!("{home_full} vs. {away_full}")
        } else {
            format!("{away_full} @ {home_full}")
        };

        let ((away_pitcher_stats, away_pitcher_id), (home_pitcher_stats, home_pitcher_id)) = pitcher_future.join().ok().context("Pitcher lines thread panicked")??;

        Ok((Lineup::new(
            datetime,
            title,
            time,
            previous,
            record,
            standings,
            away_pitcher_stats,
            home_pitcher_stats,
            first_hitting_stat,
            second_hitting_stat,
            get_last_lineup_underscores(&previous_team_lineup)?,
        ), next_game, (home_pitcher_id, away_pitcher_id)))
    })
}

fn response_parsed_values(
    response: &Value,
    home: bool,
    game_id: i64,
) -> Result<(Option<Value>, Standings, RecordAgainst, Option<NextGame>)> {
    let (our_id, our_abbreviation) = (
        response["gameData"]["teams"][if home { "home" } else { "away" }]["id"]
            .as_i64()
            .context("The selected team didn't have an id")?,
        modify_abbreviation(&response["gameData"]["teams"][if home { "home" } else { "away" }])?,
    );
    let (their_id, their_abbreviation) = (
        response["gameData"]["teams"][if home { "away" } else { "home" }]["id"]
            .as_i64()
            .context("The selected team didn't have an id")?,
        modify_abbreviation(&response["gameData"]["teams"][if home { "away" } else { "home" }])?
    );
    let game_type = response["gameData"]["game"]["type"].as_str().context("Could not get game type")?;

    let all_games_root = get(&format!("https://statsapi.mlb.com/api/v1/schedule/games/?sportId=1&startDate={year}-01-01&endDate={year}-12-31&hydrate=venue(timezone)", year = Local::now().date_naive().year()))?;
    let all_games = all_games_root["dates"].as_array().iter().flat_map(|x| x.iter()).flat_map(|game| game["games"].as_array()).flat_map(|x| x.iter()).filter(|game| (game["teams"]["home"]["team"]["id"].as_i64().is_some_and(|id| id == our_id) || game["teams"]["away"]["team"]["id"].as_i64().is_some_and(|id| id == our_id)) && game["gameType"].as_str().is_some_and(|r#type| r#type == game_type)).collect::<Vec<_>>();

    let previous_game_id = all_games.iter().rev().skip_while(|game| game["gamePk"].as_i64().map_or(true, |id| id != game_id)).skip(1).next().and_then(|game| game["gamePk"].as_i64());

    let previous_game = if let Some(previous_game_id) = previous_game_id {
        Some(get(&format!("https://statsapi.mlb.com/api/v1.1/game/{previous_game_id}/feed/live"))?)
    } else {
        None
    };

    let mut record = RecordAgainst::new(&our_abbreviation, &their_abbreviation);
    let mut standings = Standings::new();
    let mut games_played = FxHashSet::<i64>::with_capacity_and_hasher(162, Default::default());

    let next_game = if let Some(game) = all_games.iter()
        .skip_while(|game| game["gamePk"].as_i64().map_or(true, |id| id != game_id))
        .skip(1)
        .next()
        .map(|game| NextGame::new(game, our_id)) {
        Some(game?)
    } else {
        None
    };
    for game in all_games
        .iter()
        .take_while(|game| game["gamePk"].as_i64().map_or(true, |id| id != game_id))
        .filter(|game| game["status"]["codedGameState"].as_str() == Some("F"))
        .filter(|game| game["gamePk"].as_i64().map_or(true, |id| games_played.insert(id))) {
        let home_id = game["teams"]["home"]["team"]["id"]
            .as_i64()
            .context("Home Team didn't have an ID")?;
        let away_id = game["teams"]["away"]["team"]["id"]
            .as_i64()
            .context("Away Team didn't have an ID")?;
        let matchup = home_id == their_id || away_id == their_id;
        let home_score = game["teams"]["home"]["score"].as_i64().unwrap_or(0);
        let away_score = game["teams"]["away"]["score"].as_i64().unwrap_or(0);

        if home_score == away_score {
            continue
        }

        if (home_score > away_score) ^ (home_id == our_id) {
            if matchup { record.loss(); }
            standings.loss();
        } else {
            if matchup { record.win(); }
            standings.win();
        }
    }

    Ok((previous_game, standings, record, next_game))
}

pub fn get_pitcher_lines(
    response: &Value,
    home_abbreviation: &str,
    away_abbreviation: &str,
) -> Result<((PitcherLineupEntry, i64), (PitcherLineupEntry, i64))> {
    let home_pitcher_id = response["gameData"]["probablePitchers"]["home"]["id"]
        .as_i64()
        .context("Error obtaining Home Pitcher ID")?;
    let home_pitcher = response["gameData"]["probablePitchers"]["home"]["fullName"]
        .as_str()
        .context("Error obtaining Home Pitcher name")?;

    let away_pitcher_id = response["gameData"]["probablePitchers"]["away"]["id"]
        .as_i64()
        .context("Error obtaining Away Pitcher ID")?;
    let away_pitcher = response["gameData"]["probablePitchers"]["away"]["fullName"]
        .as_str()
        .context("Error obtaining Away Pitcher name")?;

    let (home_era, home_ip, home_hand) = pitching_stats(get(&format!("https://statsapi.mlb.com/api/v1/people/{home_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?)?;
    let (away_era, away_ip, away_hand) = pitching_stats(get(&format!("https://statsapi.mlb.com/api/v1/people/{away_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?)?;

    let away_pitcher_stats = PitcherLineupEntry::new(away_pitcher.to_owned(), away_abbreviation.to_owned(), away_hand, away_era, away_ip);
    let home_pitcher_stats = PitcherLineupEntry::new(home_pitcher.to_owned(), home_abbreviation.to_owned(), home_hand, home_era, home_ip);

    Ok((
        (away_pitcher_stats, away_pitcher_id),
        (home_pitcher_stats, home_pitcher_id),
    ))
}
