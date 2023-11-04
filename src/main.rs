#![feature(inline_const)]

use core::ffi::c_void;
use core::fmt::Write;
use core::str::FromStr;
use std::io::{stderr, stdout};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;
use chrono_tz::Tz::Canada__Eastern;
use fxhash::FxHashMap;
use serde_json::Value;

use crate::posts::defensive_substitution::DefensiveSubstitution;
use crate::posts::defensive_switch::DefensiveSwitch;
use crate::posts::offensive_substitution::OffensiveSubstitution;
use crate::posts::pitching_substitution::PitchingSubstitution;
use crate::posts::scoring_play::ScoringPlay;
use crate::posts::scoring_play_event::ScoringPlayEvent;
use crate::util::{clear_screen, last_name};
use crate::util::ffi::{_getch, ConsoleCursorInfo, Coordinate, GetConsoleWindow, GetStdHandle, SetConsoleCursorInfo, SetConsoleCursorPosition, SetConsoleTextAttribute, SetForegroundWindow};
use crate::util::record_against::RecordAgainst;
use crate::util::standings::Standings;
use crate::util::statsapi::{era, lineup, real_abbreviation, title, write_last_lineup_underscored};

pub mod util;

pub mod posts {
    pub mod pitching_substitution;
    pub mod scoring_play_event;
    pub mod offensive_substitution;
    pub mod defensive_substitution;
    pub mod scoring_play;
    pub mod defensive_switch;
}

fn main() {
    loop {
        if let Err(e) = unsafe { main0(GetConsoleWindow().cast()) } {
            eprintln!("Error while stalking lineup: {e}");
            eprint!("Press any key to continue... ");
            let _ = std::io::Write::flush(&mut stderr());
            unsafe { _getch() };
            clear_screen(128);
            unsafe { SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: 0 }); }
        }
    }
}

unsafe fn main0(hwnd: *mut c_void) -> Result<()> {
    SetConsoleCursorInfo(
        GetStdHandle(-11_i32 as u32),
        &ConsoleCursorInfo::new(1, false),
    );
    let (id, home) = get_id()?;
    let url = format!("https://statsapi.mlb.com/api/v1.1/game/{id}/feed/live");
    SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: 0 });
    let mut response =
        ureq::get(&url)
            .call()
            .context(anyhow!("Initial URL request failed ({url})"))?
            .into_json::<Value>().context("Initial URL Request did not return valid json")?;
    let utc = DateTime::<Utc>::from_str(response["gameData"]["datetime"]["dateTime"].as_str().context("Game Date Time didn't exist")?)?.naive_utc();
    let datetime = Canada__Eastern.from_utc_datetime(&utc);
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
    writeln!(out, "### __Starting Lineup (.AVG *|* .SLG)__")?;
    let lines_before_lineup = out.split("\n").count() - 1;
    write_last_lineup_underscored(&mut out, &previous_team_loadout)?;
    write!(out, "> ")?;
    println!("{out}\n\n\n");
    cli_clipboard::set_contents(out.clone()).map_err(|_| anyhow!("Failed to set clipboard"))?;
    {
        let mut dots = 0;
        SetConsoleCursorInfo(
            GetStdHandle(-11_i32 as u32),
            &ConsoleCursorInfo::new(1, false),
        );
        loop {
            if response["liveData"]["boxscore"]["teams"][if home { "home" } else { "away" }]
                ["battingOrder"]
                .as_array()
                .map_or(true, Vec::is_empty)
            {
                print!("\rLoading{: <pad$}", ".".repeat(dots + 1), pad = (3 - dots));
                std::io::Write::flush(&mut stdout())?;
                dots = (dots + 1) % 3;
                response =
                    match ureq::get(&url).call() {
                        Ok(response) => response,
                        Err(_) => {
                            std::thread::sleep(Duration::new(10, 0));
                            continue;
                        }
                    }
                        .into_json::<Value>().context("Response was not a valid json")?;
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
        let lineup = lineup(&response["liveData"]["boxscore"]["teams"][if home { "home" } else { "away" }], &previous_team_loadout)?;
        let mut lines = out.split("\n").map(str::to_owned).collect::<Vec<_>>();
        for (idx, line) in lineup.split("\n").map(str::to_owned).enumerate() {
            println!("{line}");
            lines[lines_before_lineup + idx] = line;
        }
        out = lines.join("\n");
        cli_clipboard::set_contents(out).map_err(|_| anyhow!("Failed to set clipboard"))?;
        let _ = std::io::Write::flush(&mut stdout())?;
    }
    SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: lines_before_lineup as i16 + 9 });
    print!("\n\n");
    SetForegroundWindow(hwnd);
    post_lineup(
        response,
        home,
        standings,
        record,
        home_pitcher_id,
        away_pitcher_id,
    )?;
    Ok(())
}

fn get_id() -> Result<(usize, bool)> {
    fn print_game(game: &Value, idx: usize, handle: *mut c_void, idx_width: usize, default_color: u16) -> Result<()> {
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
        if home == "Toronto Blue Jays" || away == "Toronto Blue Jays" {
            unsafe {
                SetConsoleTextAttribute(handle, 3);
            }
        } else {
            unsafe {
                SetConsoleTextAttribute(handle, default_color);
            }
        }
        println!(
            "  {idx: >idx_width$}. {home} vs. {away} @ {}",
            Canada__Eastern
                .from_local_datetime(&time.naive_local())
                .latest()
                .context("Error converting to Canada Eastern Timezone")?
                .format("%H:%M %Z")
        );
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
        let response =
            &ureq::get(&format!(
                "https://statsapi.mlb.com/api/v1/schedule/games/?sportId=1&date={}",
                date.format("%m/%d/%Y")
            ))
                .call()?
                .into_json::<Value>()?;
        let games = response["dates"][0]["games"]
            .as_array()
            .unwrap_or(const { &vec![] });
        let mut ids = Vec::with_capacity(games.len());
        let idx_width = (games.len() + 1).checked_ilog10().map_or(1, |x| x + 1) as usize;
        println!("[{}] Please select a game ordinal to wait on for lineups (use arrows for movement and dates): \n", date.format("%m/%d/%Y"));
        for (idx, game) in games.iter().enumerate() {
            ids.push(game["gamePk"].as_i64().context("Game ID didn't exist")?);
            print_game(game, idx, handle, idx_width, 7)?;
        }
        unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 2 }); }
        print!("> ");
        std::io::Write::flush(&mut stdout())?;
        unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 2 }); }
        loop {
            let first = unsafe { _getch() };
            if first == 0xE0 {
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
                    date = date
                        .pred_opt()
                        .context("Error when getting previous date")?;
                    clear_screen(ids.len() + 2);
                    unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                    continue 'a;
                } else if second == 0x4D {
                    idx = 0;
                    date = date
                        .succ_opt()
                        .context("Error when getting next date")?;
                    clear_screen(ids.len() + 2);
                    unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                    continue 'a;
                } else {
                    println!("{second:x}");
                    loop {}
                }
            } else if first == 0x0D {
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 2 }); }
                for (idx, game) in games.iter().enumerate() {
                    print_game(game, idx, handle, idx_width, 8)?;
                }
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                println!("[{}] Please select the home team or away team (use arrows for switching):                                \n", date.format("%m/%d/%Y"));
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
                if home == "Toronto Blue Jays" || away == "Toronto Blue Jays" {
                    unsafe { SetConsoleTextAttribute(handle, 3); }
                }
                println!(
                    "> {home} vs. {away} @ {}                                ",
                    Canada__Eastern
                        .from_local_datetime(&time.naive_local())
                        .latest()
                        .context("Error converting to Canada Eastern Timezone")?
                        .format("%H:%M %Z")
                );
                if home == "Toronto Blue Jays" || away == "Toronto Blue Jays" {
                    unsafe { SetConsoleTextAttribute(handle, 7); }
                }
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
                        return Ok((ids[idx] as usize, is_home))
                    }
                }
            }
        }
    }
}

unsafe fn post_lineup(
    response: Value,
    home: bool,
    mut standings: Standings,
    mut record: RecordAgainst,
    mut home_pitcher_id: i64,
    mut away_pitcher_id: i64,
) -> Result<()> {
    let game_id = response["gamePk"].as_i64().context("Could not get game id")?;
    let home_abbreviation = real_abbreviation(&response["gameData"]["teams"]["home"])?;
    let away_abbreviation = real_abbreviation(&response["gameData"]["teams"]["away"])?;
    let starting_home_pitcher_id = home_pitcher_id;
    let starting_away_pitcher_id = away_pitcher_id;
    let id_to_object = response["liveData"]["boxscore"]["teams"]["home"]["players"]
        .as_object()
        .context("Could not find home players list")?
        .values()
        .chain(
            response["liveData"]["boxscore"]["teams"]["away"]["players"]
                .as_object()
                .context("Could not find away players list")?
                .values(),
        )
        .filter_map(|player| player["person"]["id"].as_i64().map(|id| (id, player.clone())))
        .collect::<FxHashMap<_, _>>();
    let all_player_names = id_to_object
        .values()
        .filter_map(|obj| obj["person"]["fullName"].as_str().map(ToOwned::to_owned))
        .collect::<Vec<String>>();
    let mut scoring_plays = String::new();
    let mut previous_play_end = 0;

    let mut home_walks = 0_usize;
    let mut home_strikeouts = 0_usize;
    let mut home_receiving_pitches = 0_usize;
    let mut home_pitchers = last_name(id_to_object.get(&starting_home_pitcher_id).context("Pitcher's name should exist")?["person"]["fullName"].as_str().context("Expected pitcher's name")?).to_owned();

    let mut away_walks = 0_usize;
    let mut away_strikeouts = 0_usize;
    let mut away_receiving_pitches = 0_usize;
    let mut away_pitchers = last_name(id_to_object.get(&starting_away_pitcher_id).context("Pitcher's name should exist")?["person"]["fullName"].as_str().context("Expected pitcher's name")?).to_owned();

    let mut first_time_around = true;
    loop {
        if !core::mem::replace(&mut first_time_around, false) {
            std::thread::sleep(Duration::new(2, 0));
        }
        let Some(pbp) = ureq::get(&format!("https://statsapi.mlb.com/api/v1/game/{game_id}/playByPlay"))
            .call()
            .ok()
            .and_then(|x| x.into_json::<Value>().ok())
        else {
            std::thread::sleep(Duration::new(3, 0));
            continue;
        };
        let all_plays = pbp["allPlays"].as_array().context("Game must have a list of plays")?;
        let mut play_idx = 0_usize;
        for play in all_plays {
            // idk why it doesn't invert here, I seriously don't know what I did wrong.
            let away = play["about"]["isTopInning"]
                .as_bool()
                .unwrap();
            for play_event in play["playEvents"]
                .as_array()
                .unwrap()
            {
                if play_event["type"]
                    .as_str().unwrap()
                    == "action"
                {
                    match play_event["details"]["eventType"]
                        .as_str()
                        .unwrap()
                    {
                        "pitch" => {
                            if away {
                                away_receiving_pitches += 1;
                            } else {
                                home_receiving_pitches += 1;
                            }
                        }
                        "pitching_substitution" => {
                            if play_idx < previous_play_end {
                                play_idx += 1;
                                continue;
                            }
                            let previous_pitcher_id = if away {
                                home_pitcher_id
                            } else {
                                away_pitcher_id
                            };
                            let pitching_substitution = PitchingSubstitution::from_play(
                                play_event,
                                if away { &home_abbreviation } else { &away_abbreviation },
                                ureq::get(&format!("https://statsapi.mlb.com/api/v1/people/{previous_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])")).call()?.into_json::<Value>()?
                            )?;
                            if away {
                                home_pitcher_id = pitching_substitution.new_id();
                                home_pitchers += "/";
                                home_pitchers += pitching_substitution.last_name();
                            } else {
                                away_pitcher_id = pitching_substitution.new_id();
                                away_pitchers += "/";
                                away_pitchers += pitching_substitution.last_name();
                            }
                            println!("{pitching_substitution:?}\n\n\n");
                            cli_clipboard::set_contents(format!("{pitching_substitution:?}")).map_err(|_| anyhow!("Failed to set clipboard"))?;
                            println!("\n\n");
                            play_idx += 1;
                        }
                        "offensive_substitution" => {
                            if play_idx < previous_play_end {
                                play_idx += 1;
                                continue;
                            }
                            let offensive_substitution = OffensiveSubstitution::from_play(
                                play_event,
                                play,
                                if away {
                                    &away_abbreviation
                                } else {
                                    &home_abbreviation
                                },
                                &id_to_object,
                            )?;
                            println!("{offensive_substitution:?}\n\n\n");
                            cli_clipboard::set_contents(format!("{offensive_substitution:?}")).map_err(|_| anyhow!("Failed to set clipboard"))?;
                            play_idx += 1;
                        }
                        "defensive_substitution" => {
                            if play_idx < previous_play_end {
                                play_idx += 1;
                                continue;
                            }
                            let defensive_substitution = DefensiveSubstitution::from_play(
                                play_event,
                                play,
                                if away {
                                    &home_abbreviation
                                } else {
                                    &away_abbreviation
                                },
                                &id_to_object,
                            )?;
                            println!("{defensive_substitution:?}\n\n\n");
                            cli_clipboard::set_contents(format!("{defensive_substitution:?}")).map_err(|_| anyhow!("Failed to set clipboard"))?;
                            play_idx += 1;
                        }
                        "defensive_switch" => {
                            if play_idx < previous_play_end {
                                play_idx += 1;
                                continue;
                            }
                            let defensive_switch = DefensiveSwitch::from_play(
                                play_event,
                                play,
                                if away {
                                    &home_abbreviation
                                } else {
                                    &away_abbreviation
                                },
                                &id_to_object,
                            )?;
                            println!("{defensive_switch:?}\n\n");
                            cli_clipboard::set_contents(format!("{defensive_switch:?}")).map_err(|_| anyhow!("Failed to set clipboard"))?;
                            play_idx += 1;
                        }
                        "passed_ball" | "wild_pitch"
                            if play_event["details"]["isScoringPlay"]
                                .as_bool()
                                .context("Could not find if something was a scoring play")? =>
                        {
                            if play_idx < previous_play_end {
                                play_idx += 1;
                                continue;
                            }
                            let passed_ball = ScoringPlayEvent::from_play(
                                play_event,
                                play,
                                &home_abbreviation,
                                &away_abbreviation,
                                &all_player_names,
                                "Wild pitch",
                            )?;
                            println!("{passed_ball:?}\n\n");
                            cli_clipboard::set_contents(format!("{passed_ball:?}")).map_err(|_| anyhow!("Failed to set clipboard"))?;
                            writeln!(&mut scoring_plays, "{passed_ball}")?;
                            play_idx += 1;
                        }
                        "stolen_base_home" => {
                            if play_idx < previous_play_end {
                                play_idx += 1;
                                continue;
                            }
                            let stolen_home = ScoringPlayEvent::from_play(
                                play_event,
                                play,
                                &home_abbreviation,
                                &away_abbreviation,
                                &all_player_names,
                                "Stolen base",
                            )?;
                            println!("{stolen_home:?}\n\n");
                            cli_clipboard::set_contents(format!("{stolen_home:?}")).map_err(|_| anyhow!("Failed to set clipboard"))?;
                            writeln!(&mut scoring_plays, "{stolen_home}")?;
                            play_idx += 1;
                        }
                        _ => {}
                    }
                }
            }

            if !play["about"]["isComplete"]
                .as_bool()
                .unwrap()
            {
                break;
            };
            let desc = play["result"]["description"]
                .as_str()
                .unwrap();
            if let Some("walk" | "intent_walk") = play["eventType"].as_str() {
                if away {
                    away_walks += 1;
                } else {
                    home_walks += 1;
                }
            }
            if play["eventType"].as_str() == Some("strikeout") {
                if away {
                    away_strikeouts += 1;
                } else {
                    home_strikeouts += 1;
                }
            }
            if !(desc.contains("home run") || desc.contains("homers") || desc.contains("scores")) {
                play_idx += 1;
                continue;
            };
            if play_idx < previous_play_end {
                play_idx += 1;
                continue;
            }
            // intentionally break early here so next-time we rescan this
            let scoring = ScoringPlay::from_play(
                play,
                &home_abbreviation,
                &away_abbreviation,
                &all_player_names,
            )?;
            println!("{scoring:?}\n\n");
            writeln!(&mut scoring_plays, "{scoring}")?;
            cli_clipboard::set_contents(format!("{scoring:?}")).map_err(|_| anyhow!("Failed to set clipboard"))?;
            play_idx += 1;
        }

        previous_play_end = play_idx;

        let r = ureq::get(&format!("https://statsapi.mlb.com/api/v1/game/{game_id}/linescore")).call()?.into_json::<Value>()?;
        let innings = r["innings"]
            .as_array()
            .context("Could not get innings")?;
        if innings.len() >= 9 {
            let top = r["isTopInning"]
                .as_bool()
                .context("Could not find out if it's the top of the inning")?;
            let mut home_runs = 0;
            let mut home_hits = 0;
            let mut home_errors = 0;

            let mut away_runs = 0;
            let mut away_hits = 0;
            let mut away_errors = 0;

            for inning in innings {
                home_runs += inning["home"]["runs"].as_i64().unwrap_or(0);
                home_hits += inning["home"]["hits"].as_i64().unwrap_or(0);
                home_errors += inning["home"]["errors"].as_i64().unwrap_or(0);
                away_runs += inning["away"]["runs"].as_i64().unwrap_or(0);
                away_hits += inning["away"]["hits"].as_i64().unwrap_or(0);
                away_errors += inning["away"]["errors"].as_i64().unwrap_or(0);
            }

            let finished = if home_runs > away_runs
                && r["outs"]
                    .as_i64()
                    .context("Could not get outs for the inning")?
                    >= 3
            {
                true
            } else if away_runs > home_runs
                && !top
                && r["outs"]
                    .as_i64()
                    .context("Could not get outs for the inning")?
                    >= 3
            {
                true
            } else {
                false
            };

            if finished {
                let away_bold = if away_runs > home_runs { "**" } else { "" };
                let home_bold = if home_runs > away_runs { "**" } else { "" };
                let walkoff = if home_runs > away_runs
                    && home_runs
                        - innings
                            .last()
                            .context("You gotta have at least one inning if the game is over")?
                            ["home"]["runs"]
                            .as_i64()
                            .unwrap_or(0)
                        <= away_runs
                {
                    "**"
                } else {
                    ""
                };
                let mut header = "`    ".to_owned();
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
                header.push_str("|| R | H | E |`");
                write!(
                    &mut away_linescore,
                    "||{r: ^3}|{h: ^3}|{e: ^3}|`",
                    r = away_runs,
                    h = away_hits,
                    e = away_errors
                )?;
                write!(
                    &mut home_linescore,
                    "||{r: ^3}|{h: ^3}|{e: ^3}|`",
                    r = home_runs,
                    h = home_hits,
                    e = home_errors
                )?;

                if (away_runs > home_runs) ^ home {
                    standings.win();
                    record.add_newer_win();
                } else {
                    standings.loss();
                    record.add_newer_loss();
                }

                let pitching_masterpiece = {
                    let mut out = String::new();

                    {
                        let home_masterpiece_kind = if away_hits == 0 {
                            if away_walks == 0 {
                                Some("Perfect Game")
                            } else {
                                Some("No-Hitter")
                            }
                        } else if starting_home_pitcher_id == home_pitcher_id {
                            if away_runs == 0 {
                                Some("Complete Game Shutout")
                            } else {
                                Some("Complete Game")
                            }
                        } else {
                            None
                        };
                        if let Some(home_masterpiece_kind) = home_masterpiece_kind {
                            writeln!(out, "### {home_abbreviation} {combined}{home_masterpiece_kind}", combined = if starting_home_pitcher_id != home_pitcher_id { "Combined " } else { "" })?;
                            writeln!(out, ":star: __{home_pitchers}'s Final Line__ :star:")?;
                            writeln!(out, "> **{innings_count}.0** IP | **{away_hits}** H | **{away_runs}** ER | **{away_walks}** BB | {away_strikeouts} K", innings_count = innings.len())?;
                            writeln!(out, "> Pitch Count: **{away_receiving_pitches}**")?;
                        }
                    }

                    {
                        let away_masterpiece_kind = if home_hits == 0 {
                            if home_walks == 0 {
                                Some("Perfect Game")
                            } else {
                                Some("No-Hitter")
                            }
                        } else if starting_away_pitcher_id == away_pitcher_id {
                            if home_runs == 0 {
                                Some("Complete Game Shutout")
                            } else {
                                Some("Complete Game")
                            }
                        } else {
                            None
                        };
                        if let Some(away_masterpiece_kind) = away_masterpiece_kind {
                            writeln!(out, "### {away_abbreviation} {combined}{away_masterpiece_kind}", combined = if starting_away_pitcher_id != away_pitcher_id { "Combined " } else { "" })?;
                            writeln!(out, ":star: __{away_pitchers}'s Final Line__ :star:")?;
                            writeln!(out, "> **{innings_count}.0** IP | **{home_hits}** H | **{home_runs}** ER | **{home_walks}** BB | {home_strikeouts} K", innings_count = innings.len())?;
                            writeln!(out, "> Pitch Count: **{home_receiving_pitches}**")?;
                        }
                    }

                    out
                };

                let mut out = String::new();
                writeln!(out, "## Final Score")?;
                writeln!(out, "{away_bold}{away_abbreviation}{away_bold} {away_runs}-{walkoff}{home_runs}{walkoff} {home_bold}{home_abbreviation}{home_bold}")?;
                writeln!(out, "Standings: {standings}")?;
                writeln!(out, "Record Against: {record}")?;
                write!(out, "{pitching_masterpiece}")?;
                writeln!(out, "### __Line Score__")?;
                writeln!(out, "{header}")?;
                writeln!(out, "{away_linescore}")?;
                writeln!(out, "{home_linescore}")?;
                writeln!(out, "### __Scoring Plays__")?;
                writeln!(out, "{scoring_plays}")?;
                write!(out, "> ")?;

                println!("{out}");
                cli_clipboard::set_contents(out).map_err(|_| anyhow!("Failed to set clipboard"))?;

                loop {
                    std::thread::sleep(Duration::new(u64::MAX, 0));
                    core::hint::spin_loop();
                }
            }
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

        let (previous_game, standings, record) = standings_and_record(&response, home, game_id)?;

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
        let title = title(home, home_full, away_full);

        let ((away_pitcher_line, away_pitcher_id), (home_pitcher_line, home_pitcher_id)) = pitcher_future.join().ok().context("Pitcher lines thread panicked")??;

        Ok((
            title,
            previous,
            record,
            standings,
            (away_pitcher_line, away_pitcher_id),
            (home_pitcher_line, home_pitcher_id),
            previous_team_loadout,
        ))
    })
}

fn standings_and_record(
    response: &Value,
    home: bool,
    game_id: i64,
) -> Result<(Option<Value>, Standings, RecordAgainst)> {
    let (our_id, our_abbreviation) = (
        response["gameData"]["teams"][if home { "home" } else { "away" }]["id"]
            .as_i64()
            .context("The selected team didn't have an id")?,
        real_abbreviation(&response["gameData"]["teams"][if home { "home" } else { "away" }])?,
    );
    let (their_id, their_abbreviation) = (
        response["gameData"]["teams"][if home { "away" } else { "home" }]["id"]
            .as_i64()
            .context("The opponent team didn't have an id")?,
        real_abbreviation(&response["gameData"]["teams"][if home { "away" } else { "home" }])?,
    );
    let team_response = ureq::get(&format!("https://statsapi.mlb.com/api/v1/teams/{our_id}?hydrate=previousSchedule(limit=2147483647)")).call()?.into_json::<Value>()?;

    let previous_game_id = team_response["teams"][0]["previousGameSchedule"]["dates"]
        .as_array()
        .context("Team didn't have previous games")?
        .iter()
        .flat_map(|games| games["games"].as_array())
        .flat_map(|x| x.iter())
        .rev()
        .skip_while(|game| game["gamePk"].as_i64().map_or(true, |id| id != game_id))
        .nth(1)
        .and_then(|game| game["gamePk"].as_i64());

    let previous_game = if let Some(previous_game_id) = previous_game_id {
        Some(ureq::get(&format!(
            "https://statsapi.mlb.com/api/v1.1/game/{previous_game_id}/feed/live"
        ))
            .call()?
            .into_json()?)
    } else {
        None
    };

    let (wins, losses) = if home {
        (
            response["gameData"]["teams"]["home"]["record"]["wins"]
                .as_i64()
                .context("Home Team didn't have a wins count")?,
            response["gameData"]["teams"]["home"]["record"]["losses"]
                .as_i64()
                .context("Home Team didn't have a losses count")?,
        )
    } else {
        (
            response["gameData"]["teams"]["away"]["record"]["wins"]
                .as_i64()
                .context("Away Team didn't have a wins count")?,
            response["gameData"]["teams"]["away"]["record"]["losses"]
                .as_i64()
                .context("Away Team didn't have a losses count")?,
        )
    };

    let mut record = RecordAgainst::new(&our_abbreviation, &their_abbreviation);
    let mut standings = Standings::new(wins, losses);
    let mut streak_end = false;

    let games = team_response["teams"][0]["previousGameSchedule"]["dates"]
        .as_array()
        .context("Team didn't have any previous games")?;
    for game in games
        .iter()
        .flat_map(|game| game["games"].as_array())
        .flat_map(|x| x.iter())
        .rev()
        .skip_while(|game| game["gamePk"].as_i64().map_or(true, |id| id != game_id))
        .skip(1)
    {
        let home_id = game["teams"]["home"]["team"]["id"]
            .as_i64()
            .context("Home Team didn't have an ID")?;
        let away_id = game["teams"]["away"]["team"]["id"]
            .as_i64()
            .context("Away Team didn't have an ID")?;

        if !streak_end {
            let home_score = game["teams"]["home"]["score"].as_i64().context("Home Team didn't have a score")?;
            let away_score = game["teams"]["away"]["score"].as_i64().context("Away Team didn't have a score")?;
            let (our_score, their_score) = if home_id == our_id {
                (home_score, away_score)
            } else {
                (away_score, home_score)
            };
            streak_end = !if our_score > their_score {
                standings.streak_older_win()
            } else {
                standings.streak_older_loss()
            };
        }

        if home_id == our_id && away_id == their_id || home_id == their_id && away_id == our_id {
            let home_score = game["teams"]["home"]["score"].as_i64().context("Home Team didn't have a score")?;
            let away_score = game["teams"]["away"]["score"].as_i64().context("Away Team didn't have a score")?;
            if home_score > away_score {
                if home_id == our_id {
                    record.add_older_win();
                } else {
                    record.add_older_loss();
                }
            } else {
                if home_id == our_id {
                    record.add_older_loss();
                } else {
                    record.add_older_win();
                }
            }
        }
    }

    Ok((previous_game, standings, record))
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

    let (home_era, home_l7) = era(ureq::get(&format!("https://statsapi.mlb.com/api/v1/people/{home_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])")).call()?.into_json::<Value>()?)?;
    let (away_era, away_l7) = era(ureq::get(&format!("https://statsapi.mlb.com/api/v1/people/{away_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])")).call()?.into_json::<Value>()?)?;

    let away_pitcher_line =
        format!("{away_abbreviation}: {away_pitcher} ({away_era:.2} ERA *|* {away_l7:.2} L7)");
    let home_pitcher_line =
        format!("{home_abbreviation}: {home_pitcher} ({home_era:.2} ERA *|* {home_l7:.2} L7)");

    Ok((
        (away_pitcher_line, away_pitcher_id),
        (home_pitcher_line, home_pitcher_id),
    ))
}
