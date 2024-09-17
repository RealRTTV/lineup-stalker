#![feature(lazy_cell)]

use core::ffi::c_void;
use core::str::FromStr;
use std::io::{stderr, stdout};
use core::fmt::Write as OtherWrite;
use std::convert::identity;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{Datelike, DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;
use chrono_tz::Tz::America__Toronto;
use fxhash::FxHashMap;
use serde_json::Value;

use crate::posts::defensive_substitution::DefensiveSubstitution;
use crate::posts::defensive_switch::DefensiveSwitch;
use crate::posts::offensive_substitution::OffensiveSubstitution;
use crate::posts::pitching_substitution::PitchingSubstitution;
use crate::posts::Post;
use crate::posts::scoring_play::ScoringPlay;
use crate::posts::scoring_play_event::ScoringPlayEvent;
use crate::util::{clear_screen, get_team_color_escape};
use crate::util::decisions::Decisions;
use crate::util::fangraphs::{BALLPARK_ADJUSTMENTS, WOBA_CONSTANTS};
use crate::util::ffi::{_getch, ConsoleCursorInfo, Coordinate, GetStdHandle, SetConsoleCursorInfo, SetConsoleCursorPosition, SetConsoleTextAttribute};
use crate::util::next_game::NextGame;
use crate::util::record_against::RecordAgainst;
use crate::util::standings::Standings;
use crate::util::stat::HittingStat;
use crate::util::statsapi::{pitching_stats, lineup, real_abbreviation, write_last_lineup_underscored};
use crate::util::team_stats_log::TeamStatsLog;

pub const TIMEZONE: Tz = America__Toronto;

pub mod util;
pub mod posts;

fn main() {
    let _ = WOBA_CONSTANTS.deref();
    let _ = BALLPARK_ADJUSTMENTS.deref();

    loop {
        clear_screen(128);
        unsafe { SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: 0 }); }
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

unsafe fn main0() -> Result<()> {
    SetConsoleCursorInfo(
        GetStdHandle(-11_i32 as u32),
        &ConsoleCursorInfo::new(1, false),
    );
    let (id, home, first_stat, second_stat) = get_id()?;
    let url = format!("https://statsapi.mlb.com/api/v1.1/game/{id}/feed/live");
    SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: 0 });
    let mut response = get(&url)?;
    let utc = DateTime::<Utc>::from_str(response["gameData"]["datetime"]["dateTime"].as_str().context("Game Date Time didn't exist")?)?.naive_utc();
    let datetime = TIMEZONE.from_utc_datetime(&utc);
    let local_datetime = Tz::from_str(response["gameData"]["venue"]["timeZone"]["id"].as_str().context("Could not find venue's local time zone for game")?).map_err(|e| anyhow!("{e}"))?.from_utc_datetime(&utc);
    let time = if datetime.naive_local() == local_datetime.naive_local() {
        format!("{}", datetime.format("%H:%M %Z"))
    } else {
        format!("{} / {}", datetime.format("%H:%M %Z"), local_datetime.format("%H:%M %Z"))
    };
    let game_id = response["gameData"]["game"]["pk"]
        .as_i64()
        .context("Game ID didn't exist")?;
    let (
        title,
        previous,
        record,
        standings,
        next_game,
        (away_pitcher_line, away_pitcher_id),
        (home_pitcher_line, home_pitcher_id),
        previous_team_loadout,
    ) = lines(&response, home, game_id)?;
    let mut out = String::new();
    writeln!(out, "# {} {title}", datetime.format("%m*|*%d*|*%y"))?;
    writeln!(out, "First Pitch: {time}")?;
    if let Some(previous) = previous {
        writeln!(out, "Previous Game: {previous}")?;
    }
    writeln!(out, "Record Against: {record}")?;
    writeln!(out, "Standings: {standings}")?;
    writeln!(out, "### __Starting Pitchers__")?;
    writeln!(out, "{away_pitcher_line}")?;
    writeln!(out, "{home_pitcher_line}")?;
    writeln!(out, "### __Starting Lineup (.{first_stat_value} *|* .{second_stat_value})__", first_stat_value = first_stat.to_string(), second_stat_value = second_stat.to_string())?;
    let lines_before_lineup = out.split("\n").count() - 1;
    write_last_lineup_underscored(&mut out, &previous_team_loadout)?;
    write!(out, "> ")?;
    Post::Lineup { has_lineup: false }.send(&out)?;
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
    SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: lines_before_lineup as i16 });
    {
        let lineup = lineup(&response["liveData"]["boxscore"]["teams"][if home { "home" } else { "away" }], first_stat, second_stat, response["gameData"]["game"]["type"].as_str().context("Expected game type")? != "R" || response["gameData"]["game"]["type"].as_str().context("Expected game type")? != "S", &response["gameData"]["teams"][if home { "home" } else { "away" }]["teamName"].as_str().context("Expected team name")?)?;
        let mut lines = out.split("\n").map(str::to_owned).collect::<Vec<_>>();
        for (idx, line) in lineup.split("\n").map(str::to_owned).enumerate() {
            println!("{line}                                                                ");
            lines[lines_before_lineup + idx] = line;
        }
        out = lines.join("\n");
        Post::Lineup { has_lineup: true }.send_with_settings(&out, false, true, true)?;
    }
    SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: lines_before_lineup as i16 + 9 });
    print!("\n\n");
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
    fn print_game(game: &Value, idx: usize, handle: *mut c_void, idx_width: usize, default_color_escape: &str, always_use_default_color: bool) -> Result<()> {
        let idx = idx + 1;
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
        let timestamp = TIMEZONE
            .from_local_datetime(&time.naive_local())
            .latest()
            .context("Error converting to timezone")?
            .format("%H:%M %Z");
        if always_use_default_color {
            println!("\x1B[{default_color_escape}m  {idx: >idx_width$}. {home} vs. {away} @ {timestamp}\x1B[0m");
        } else {
            println!(
                "  {idx: >idx_width$}. \x1B[{home_color_escape}m{home}\x1B[0m vs. \x1B[{away_color_escape}m{away}\x1B[0m @ {timestamp}",
                home_color_escape = get_team_color_escape(home),
                away_color_escape = get_team_color_escape(away),
            );
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
        unsafe {
            SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: 0 });
        }
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
            print_game(game, idx, handle, idx_width, "0", false)?;
        }
        unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 2 }); }
        print!("> ");
        std::io::Write::flush(&mut stdout())?;
        unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 2 }); }
        loop {
            let first = unsafe { _getch() };
            if first == 0x33 {
                idx = 0;
                date = date.with_month(3).context("Error when setting month to march")?;
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }

                continue 'a;
            } else if first == 0x34 {
                idx = 0;
                date = date.with_month(4).context("Error when setting month to april")?;
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                continue 'a;
            } else if first == 0x35 {
                idx = 0;
                date = date.with_month(5).context("Error when setting month to may")?;
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                continue 'a;
            } else if first == 0x36 {
                idx = 0;
                date = date.with_month(6).context("Error when setting month to june")?;
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                continue 'a;
            } else if first == 0x37 {
                idx = 0;
                date = date.with_month(7).context("Error when setting month to july")?;
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                continue 'a;
            } else if first == 0x38 {
                idx = 0;
                date = date.with_month(8).context("Error when setting month to august")?;
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                continue 'a;
            } else if first == 0x39 {
                idx = 0;
                date = date.with_month(9).context("Error when setting month to september")?;
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                continue 'a;
            } else if first == 0x30 {
                idx = 0;
                date = date.with_month(10).context("Error when setting month to october")?;
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                continue 'a;
            } else if first == 0xE0 {
                let second = unsafe { _getch() };
                if second == 0x48 {
                    unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: idx as i16 + 2 }); }
                    print!("  ");
                    std::io::Write::flush(&mut stdout())?;
                    idx = idx.saturating_sub(1);
                    unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: idx as i16 + 2 }); }
                    print!("> ");
                    std::io::Write::flush(&mut stdout())?;
                } else if second == 0x50 {
                    unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: idx as i16 + 2 }); }
                    print!("  ");
                    std::io::Write::flush(&mut stdout())?;
                    idx = (idx + 1).min(ids.len() - 1);
                    unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: idx as i16 + 2 }); }
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
                    unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
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
                    unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                    continue 'a;
                } else {
                    println!("{second:x}");
                    loop {}
                }
            } else if first == 0x0D && !games.is_empty() {
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 2 }); }
                for (current_idx, game) in games.iter().enumerate() {
                    if current_idx == idx {
                        print_game(game, current_idx, handle, idx_width, "0", false)?;
                    } else {
                        print_game(game, current_idx, handle, idx_width, "90", true)?;
                    }
                    std::thread::sleep(Duration::from_millis(35 - current_idx as u64));
                }
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                println!("[{}] Please select the home team or away team (use arrows for switching):                                \n", date.format("%A, %B %e %Y"));
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: idx as i16 + 2 }); }
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
                        unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                        println!("[{}] Please select hitting stats (use arrows):                                \n", date.format("%A, %B %e %Y"));
                        let mut stats = [HittingStat::AVG, HittingStat::wRCp];
                        let mut selected_stat_idx = 0_usize;
                        loop {
                            unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 2 }); }
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
    home: bool,
    mut standings: Standings,
    mut record: RecordAgainst,
    next_game: NextGame,
    mut home_pitcher_id: i64,
    mut away_pitcher_id: i64,
) -> Result<()> {
    let game_id = response["gamePk"].as_i64().context("Could not get game id")?;
    let home_abbreviation = real_abbreviation(&response["gameData"]["teams"]["home"])?;
    let away_abbreviation = real_abbreviation(&response["gameData"]["teams"]["away"])?;
    let starting_home_pitcher_id = home_pitcher_id;
    let starting_away_pitcher_id = away_pitcher_id;
    let id_to_object = response["gameData"]["players"]
        .as_object()
        .context("Could not find home players list")?
        .values()
        .filter_map(|player| player["person"]["id"].as_i64().map(|id| (id, player.clone())))
        .collect::<FxHashMap<_, _>>();
    let all_player_names = id_to_object
        .values()
        .filter_map(|obj| obj["person"]["fullName"].as_str().map(ToOwned::to_owned))
        .collect::<Vec<String>>();
    let mut scoring_plays = Vec::new();
    let mut previous_play_plus_play_event_len = 0;

    let mut home_stats_log = TeamStatsLog::new(id_to_object.get(&starting_home_pitcher_id).context("Pitcher's name should exist")?["person"]["lastName"].as_str().context("Expected pitcher's name")?.to_owned());
    let mut away_stats_log = TeamStatsLog::new(id_to_object.get(&starting_away_pitcher_id).context("Pitcher's name should exist")?["person"]["lastName"].as_str().context("Expected pitcher's name")?.to_owned());

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
        for (away, parent, play) in all_plays
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
                    if away {
                        away_stats_log.walk();
                    } else {
                        home_stats_log.walk();
                    }
                }
                if play["result"]["eventType"].as_str() == Some("strikeout") {
                    if away {
                        away_stats_log.strikeout();
                    } else {
                        home_stats_log.strikeout();
                    }
                }
                if !(desc.contains("home run") || desc.contains("homers") || desc.contains("scores")) {
                    continue;
                };
                let scoring = ScoringPlay::from_play(
                    play,
                    &home_abbreviation,
                    &away_abbreviation,
                    &all_player_names,
                )?;
                Post::ScoringPlay.send(format!("{scoring:?}"))?;
                scoring_plays.push(scoring.to_string());
            } else {
                if play["type"].as_str().unwrap() == "pitch" {
                    if away {
                        home_stats_log.pitch_thrown();
                    } else {
                        away_stats_log.pitch_thrown();
                    }
                } else if play["type"].as_str().unwrap() == "action" {
                    match play["details"]["eventType"]
                        .as_str()
                        .unwrap()
                    {
                        "pitching_substitution" => {
                            let previous_pitcher_id = if away {
                                home_pitcher_id
                            } else {
                                away_pitcher_id
                            };
                            let pitching_substitution = PitchingSubstitution::from_play(
                                play,
                                if away { &home_abbreviation } else { &away_abbreviation },
                                get(&format!("https://statsapi.mlb.com/api/v1/people/{previous_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?
                            )?;
                            if away {
                                home_pitcher_id = pitching_substitution.new_id();
                                home_stats_log.change_pitcher(pitching_substitution.old_last_name().to_owned());
                            } else {
                                away_pitcher_id = pitching_substitution.new_id();
                                away_stats_log.change_pitcher(pitching_substitution.old_last_name().to_owned());
                            }
                            Post::PitchingSubstitution.send(format!("{pitching_substitution:?}"))?;
                        }
                        "offensive_substitution" => {
                            let offensive_substitution = OffensiveSubstitution::from_play(
                                play,
                                parent,
                                if away {
                                    &away_abbreviation
                                } else {
                                    &home_abbreviation
                                },
                                &id_to_object,
                            )?;
                            Post::OffensiveSubstitution.send(format!("{offensive_substitution:?}"))?;
                        }
                        "defensive_substitution" => {
                            let defensive_substitution = DefensiveSubstitution::from_play(
                                play,
                                parent,
                                if away {
                                    &home_abbreviation
                                } else {
                                    &away_abbreviation
                                },
                                &id_to_object,
                            )?;
                            Post::DefensiveSubstitution.send(format!("{defensive_substitution:?}"))?;
                        }
                        "defensive_switch" => {
                            let defensive_switch = DefensiveSwitch::from_play(
                                play,
                                parent,
                                if away {
                                    &home_abbreviation
                                } else {
                                    &away_abbreviation
                                },
                                &id_to_object,
                            )?;
                            Post::DefensiveSwitch.send(format!("{defensive_switch:?}"))?;
                        }
                        "passed_ball" | "wild_pitch"
                            if play["details"]["isScoringPlay"]
                                .as_bool()
                                .context("Could not find if something was a scoring play")? =>
                        {
                            let passed_ball = ScoringPlayEvent::from_play(
                                play,
                                parent,
                                &home_abbreviation,
                                &away_abbreviation,
                                &all_player_names,
                                "Wild pitch",
                            )?;
                            Post::PassedBall.send(format!("{passed_ball:?}"))?;
                            scoring_plays.push(passed_ball.to_string());
                        }
                        "stolen_base_home" => {
                            let stolen_home = ScoringPlayEvent::from_play(
                                play,
                                parent,
                                &home_abbreviation,
                                &away_abbreviation,
                                &all_player_names,
                                "Stolen base",
                            )?;
                            Post::StolenHome.send(format!("{stolen_home:?}"))?;
                            scoring_plays.push(stolen_home.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }

        previous_play_plus_play_event_len = all_plays.iter().map(|play| play["playEvents"].as_array().map_or(0, |vec| vec.len()) + play["about"]["isComplete"].as_bool().is_some_and(identity) as usize).sum();

        let response = get(&format!("https://statsapi.mlb.com/api/v1.1/game/{game_id}/feed/live"))?;
        if !response["liveData"]["decisions"]["winner"].is_null() {
            let linescore = &response["liveData"]["linescore"];
            let innings = linescore["innings"]
                .as_array()
                .context("Could not get innings")?;
            let top = linescore["isTopInning"]
                .as_bool()
                .context("Could not find out if it's the top of the inning")?;
            for inning in innings {
                home_stats_log.add_runs(inning["home"]["runs"].as_i64().unwrap_or(0) as usize);
                home_stats_log.add_hits(inning["home"]["hits"].as_i64().unwrap_or(0) as usize);
                home_stats_log.add_errors(inning["home"]["errors"].as_i64().unwrap_or(0) as usize);
                away_stats_log.add_runs(inning["away"]["runs"].as_i64().unwrap_or(0) as usize);
                away_stats_log.add_hits(inning["away"]["hits"].as_i64().unwrap_or(0) as usize);
                away_stats_log.add_errors(inning["away"]["errors"].as_i64().unwrap_or(0) as usize);
            }

            let away_bold = if away_stats_log.runs > home_stats_log.runs { "**" } else { "" };
            let home_bold = if home_stats_log.runs > away_stats_log.runs { "**" } else { "" };
            let walkoff = if home_stats_log.runs > away_stats_log.runs
                && home_stats_log.runs
                - innings
                .last()
                .context("You gotta have at least one inning if the game is over")?
                ["home"]["runs"]
                .as_i64()
                .unwrap_or(0) as usize
                <= away_stats_log.runs
            {
                "**"
            } else {
                ""
            };
            let mut header = "**`    ".to_owned();
            let mut away_linescore = format!("`{away_abbreviation: <3} ");
            let mut home_linescore = format!("`{home_abbreviation: <3} ");
            for (idx, inning) in innings.iter().enumerate() {
                write!(
                    &mut header,
                    "|{n: ^3}",
                    n = inning["num"]
                        .as_i64()
                        .context("Could not find inning number")?
                )?;
                write!(
                    &mut away_linescore,
                    "|{n: ^3}",
                    n = inning["away"]["runs"].as_i64().unwrap_or(0)
                )?;
                write!(
                    &mut home_linescore,
                    "|{n: ^3}",
                    n = if idx + 1 == innings.len() && top {
                        "-".to_owned()
                    } else {
                        inning["home"]["runs"].as_i64().unwrap_or(0).to_string()
                    }
                )?;
            }
            header.push_str("|| R | H | E |`**");
            write!(
                &mut away_linescore,
                "||{r: ^3}|{h: ^3}|{e: ^3}|`",
                r = away_stats_log.runs,
                h = away_stats_log.hits,
                e = away_stats_log.errors
            )?;
            write!(
                &mut home_linescore,
                "||{r: ^3}|{h: ^3}|{e: ^3}|`",
                r = home_stats_log.runs,
                h = home_stats_log.hits,
                e = home_stats_log.errors
            )?;

            if (away_stats_log.runs > home_stats_log.runs) ^ home {
                standings.win();
                record.win();
            } else {
                standings.loss();
                record.loss();
            }

            let pitching_masterpiece = TeamStatsLog::generate_masterpiece(&home_stats_log, &away_stats_log, innings.len(), &home_abbreviation).unwrap_or(String::new()) + &TeamStatsLog::generate_masterpiece(&away_stats_log, &home_stats_log, innings.len(), &away_abbreviation).unwrap_or(String::new());
            let decisions = Decisions::new(&response)?;

            let mut out = String::new();
            writeln!(out, "## Final Score")?;
            writeln!(out, "{away_bold}{away_abbreviation}{away_bold} {away_runs}-{walkoff}{home_runs}{walkoff} {home_bold}{home_abbreviation}{home_bold}", away_runs = away_stats_log.runs, home_runs = home_stats_log.runs)?;
            writeln!(out, "Standings: {standings}")?;
            writeln!(out, "Record Against: {record}")?;
            writeln!(out, "Next Game: {next_game}")?;
            write!(out, "{pitching_masterpiece}")?;
            writeln!(out, "### __Line Score__")?;
            writeln!(out, "{header}")?;
            writeln!(out, "{away_linescore}")?;
            writeln!(out, "{home_linescore}")?;
            writeln!(out, "### __Scoring Plays__")?;
            write!(out, "{}", scoring_plays.join("\n"))?;
            writeln!(out, "### __Pitcher Decisions__")?;
            writeln!(out, "{decisions}")?;
            write!(out, "> ")?;

            Post::FinalCard.send(&out)?;

            while !cancelled.load(Ordering::Relaxed) { core::hint::spin_loop() }
            return Ok(())
        }
    }
}

fn lines(
    response: &Value,
    home: bool,
    game_id: i64,
) -> Result<(
    String,
    Option<String>,
    RecordAgainst,
    Standings,
    NextGame,
    (String, i64),
    (String, i64),
    Value,
)> {
    let home_full = response["gameData"]["teams"]["home"]["name"]
        .as_str()
        .context("Home Team didn't have a full name")?;
    let away_full = response["gameData"]["teams"]["away"]["name"]
        .as_str()
        .context("Away Team didn't have a full name")?;

    let (home_abbreviation, away_abbreviation) = (
        real_abbreviation(&response["gameData"]["teams"]["home"])?,
        real_abbreviation(&response["gameData"]["teams"]["away"])?,
    );

    std::thread::scope(|s| {
        let pitcher_future = s.spawn(|| get_pitcher_lines(&response, &home_abbreviation, &away_abbreviation));

        let (previous_game, standings, record, next_game) = standings_record_next_game(&response, home, game_id)?;

        let (previous, previous_team_loadout) = if let Some(previous_game) = previous_game {
            let (home_score, away_score) = (
                previous_game["liveData"]["boxscore"]["teams"]["home"]["teamStats"]["batting"]["runs"]
                    .as_i64()
                    .context("Home Team didn't have runs")?,
                previous_game["liveData"]["boxscore"]["teams"]["away"]["teamStats"]["batting"]["runs"]
                    .as_i64()
                    .context("Away Team didn't have runs")?,
            );

            let home_bold = if home_score > away_score { "**" } else { "" };
            let away_bold = if away_score > home_score { "**" } else { "" };

            let (previous_home_abbreviation, previous_away_abbreviation) = (
                real_abbreviation(&previous_game["gameData"]["teams"]["home"])?,
                real_abbreviation(&previous_game["gameData"]["teams"]["away"])?,
            );

            let previous_innings = previous_game["liveData"]["linescore"]["innings"]
                .as_array()
                .context("Could not get innings")?
                .len();
            let extra_innings_suffix = if previous_innings > 9 {
                format!(" ({previous_innings})")
            } else {
                String::new()
            };
            (Some(format!("{away_bold}{previous_away_abbreviation}{away_bold} {away_score}-{home_score} {home_bold}{previous_home_abbreviation}{home_bold}{extra_innings_suffix}")), previous_game["liveData"]["boxscore"]["teams"][if home {
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
            }].clone())
        } else {
            (None, Value::Null)
        };

        let title = if home {
            format!("{home_full} vs. {away_full}")
        } else {
            format!("{away_full} @ {home_full}")
        };

        let ((away_pitcher_line, away_pitcher_id), (home_pitcher_line, home_pitcher_id)) = pitcher_future.join().ok().context("Pitcher lines thread panicked")??;

        Ok((
            title,
            previous,
            record,
            standings,
            next_game,
            (away_pitcher_line, away_pitcher_id),
            (home_pitcher_line, home_pitcher_id),
            previous_team_loadout,
        ))
    })
}

fn standings_record_next_game(
    response: &Value,
    home: bool,
    game_id: i64,
) -> Result<(Option<Value>, Standings, RecordAgainst, NextGame)> {
    let (our_id, our_abbreviation) = (
        response["gameData"]["teams"][if home { "home" } else { "away" }]["id"]
            .as_i64()
            .context("The selected team didn't have an id")?,
        real_abbreviation(&response["gameData"]["teams"][if home { "home" } else { "away" }])?,
    );
    let (their_id, their_abbreviation) = (
        response["gameData"]["teams"][if home { "away" } else { "home" }]["id"]
            .as_i64()
            .context("The selected team didn't have an id")?,
        real_abbreviation(&response["gameData"]["teams"][if home { "away" } else { "home" }])?
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

    let next_game = NextGame::new(all_games.iter()
        .skip_while(|game| game["gamePk"].as_i64().map_or(true, |id| id != game_id))
        .skip(1)
        .next()
        .context("Could not find upcoming game")?, our_id)?;
    for game in all_games.iter().rev().skip_while(|game| game["gamePk"].as_i64().map_or(true, |id| id != game_id)).skip(1).collect::<Vec<_>>().into_iter().rev() {
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
) -> Result<((String, i64), (String, i64))> {
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

    let away_pitcher_line = format!("`{away_hand}` | **{away_abbreviation}** {away_pitcher} ({away_era:.2} ERA *|* {away_ip:.1} IP)");
    let home_pitcher_line = format!("`{home_hand}` | **{home_abbreviation}** {home_pitcher} ({home_era:.2} ERA *|* {home_ip:.1} IP)");

    Ok((
        (away_pitcher_line, away_pitcher_id),
        (home_pitcher_line, home_pitcher_id),
    ))
}
