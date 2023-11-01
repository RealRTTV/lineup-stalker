use std::cmp::Ordering;
use std::ffi::c_void;
use std::fmt::{Debug, Display, Formatter, Write};
use std::io::{stdin, stdout, BufRead, stderr};
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local, TimeZone, Utc};
use chrono_tz::Tz;
use chrono_tz::Tz::Canada__Eastern;
use fxhash::FxHashMap;
use serde_json::Value;

#[repr(C)]
pub struct ConsoleCursorInfo {
    size: i32,
    visible: u32,
}

impl ConsoleCursorInfo {
    pub const fn new(size: i32, visible: bool) -> Self {
        Self {
            size,
            visible: visible as u32,
        }
    }
}

#[repr(C)]
pub struct Coordinate {
    pub x: i16,
    pub y: i16,
}

#[link(name = "kernel32")]
extern "system" {
    pub fn SetConsoleCursorInfo(handle: *mut c_void, param: *const ConsoleCursorInfo) -> bool;

    pub fn SetConsoleCursorPosition(handle: *mut c_void, pos: Coordinate) -> bool;

    pub fn SetConsoleTextAttribute(handle: *mut c_void, attributes: u16) -> bool;

    #[must_use]
    pub fn GetStdHandle(id: u32) -> *mut c_void;

    #[must_use]
    pub fn GetConsoleWindow() -> *mut c_void;
}

#[link(name = "msvcrt")]
extern "system" {
    pub fn _getch() -> u32;
}

#[link(name = "user32")]
extern "system" {
    pub fn SetForegroundWindow(hwnd: *mut c_void) -> bool;
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
    let (id, mut lines_printed) = get_id()?;
    SetConsoleCursorInfo(
        GetStdHandle(-11_i32 as u32),
        &ConsoleCursorInfo::new(1, true),
    );
    let url = format!("https://statsapi.mlb.com/api/v1.1/game/{id}/feed/live");
    #[cfg(debug_assertions)]
    println!("{url}");
    #[cfg(not(debug_assertions))]
    println!();
    lines_printed += 1;
    let home = {
        print!("Would you like to generate your template for the home team or the away team?\n> ");
        lines_printed += 1;
        std::io::Write::flush(&mut stdout())?;
        'a: loop {
            let mut str = String::new();
            SetConsoleTextAttribute(GetStdHandle(-11_i32 as u32), 2);
            stdin().lock().read_line(&mut str)?;
            lines_printed += 1;
            SetConsoleTextAttribute(GetStdHandle(-11_i32 as u32), 7);
            let str = str.trim_end().to_lowercase();
            if str == "home" {
                break 'a true;
            } else if str == "away" {
                break 'a false;
            }

            print!("Invalid value, try again\n> ");
            lines_printed += 1;
            std::io::Write::flush(&mut stdout())?;
        }
    };
    clear_screen(lines_printed);
    SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: 0 });
    let mut response =
        ureq::get(&url)
            .call()
            .context("Initial URL request failed ({url})")?
            .into_json::<Value>().context("Initial URL Request did not return valid json")?;
    let utc = DateTime::<Utc>::from_str(response["gameData"]["datetime"]["dateTime"].as_str().context("Game Date Time didn't exist")?, )?.naive_utc();
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
    SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: 8 });
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
    SetConsoleCursorPosition(GetStdHandle(-11_i32 as u32), Coordinate { x: 0, y: 18 });
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
    loop {
        std::thread::sleep(Duration::new(2, 0));
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
                .context("Could not find inning half")?;
            for play_event in play["playEvents"]
                .as_array()
                .context("Could not get play events")?
            {
                if play_event["type"]
                    .as_str()
                    .context("Could not tell type of play event")?
                    == "action"
                {
                    match play_event["details"]["eventType"]
                        .as_str()
                        .context("Could not tell event type of play event")?
                    {
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
                                home_pitcher_id = pitching_substitution.new_id;
                            } else {
                                away_pitcher_id = pitching_substitution.new_id;
                            }
                            pitching_substitution.dbg_print();
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
                .context("Could not find the complete-ness of the play")?
            {
                break;
            };
            let desc = play["result"]["description"]
                .as_str()
                .context("Could not find the description of the play")?;
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
                    #[cfg(debug_assertions)]
                    {
                        standings.wins -= 1;
                    }
                    record.add_newer_win();
                } else {
                    standings.loss();
                    #[cfg(debug_assertions)]
                    {
                        standings.losses -= 1;
                    }
                    record.add_newer_loss();
                }

                let mut out = String::new();
                writeln!(out, "## Final Score")?;
                writeln!(out, "{away_bold}{away_abbreviation}{away_bold} {away_runs}-{walkoff}{home_runs}{walkoff} {home_bold}{home_abbreviation}{home_bold}")?;
                writeln!(out, "Standings: {standings}")?;
                writeln!(out, "Record Against: {record}")?;
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

// no remapping atm
fn remap_score_event(event: &str, all_player_names: &[String]) -> String {
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

pub struct ScoringPlayEvent {
    away_abbreviation: String,
    away_score: i64,
    home_abbreviation: String,
    home_score: i64,
    inning: u8,
    outs: u8,
    top: bool,
    scores: Vec<Score>,
    event: &'static str,
}

impl ScoringPlayEvent {
    pub fn from_play(
        play: &Value,
        parent: &Value,
        home_abbreviation: &str,
        away_abbreviation: &str,
        all_player_names: &[String],
        event: &'static str,
    ) -> Result<Self> {
        Ok(Self {
            away_abbreviation: away_abbreviation.to_owned(),
            away_score: play["details"]["awayScore"]
                .as_i64()
                .context("Could not find away score")?,
            home_abbreviation: home_abbreviation.to_owned(),
            home_score: play["details"]["homeScore"]
                .as_i64()
                .context("Could not find home score")?,
            inning: parent["about"]["inning"]
                .as_i64()
                .context("Could not find inning")? as u8,
            outs: play["count"]["outs"]
                .as_i64()
                .context("Could not find outs")? as u8,
            top: parent["about"]["isTopInning"]
                .as_bool()
                .context("Could not find inning half")?,
            scores: {
                let description = play["details"]["description"]
                    .as_str()
                    .context("Could not get play description")?;
                let mut vec = Vec::new();
                let mut iter = description
                    .split_once(": ")
                    .map_or(description, |(_, x)| x)
                    .split("  ")
                    .map(str::trim)
                    .filter(|str| !str.is_empty());
                while let Some(value) = iter.next() {
                    let value = if all_player_names.iter().any(|name| value == *name) {
                        remap_score_event(
                            &format!(
                                "{value} {}",
                                iter.next().context("Play unexpectedly ended")?
                            ),
                            all_player_names,
                        )
                    } else {
                        remap_score_event(value, all_player_names)
                    };

                    vec.push(Score {
                        scoring: value.contains(" scores.")
                            || value.contains(" homers")
                            || value.contains("home run"),
                        value,
                    })
                }
                vec
            },
            event,
        })
    }
}

impl Debug for ScoringPlayEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let walkoff = !self.top && self.inning >= 9 && self.home_score > self.away_score;

        let away_abbreviation = &*self.away_abbreviation;
        let away_score = self.away_score;
        let home_abbreviation = &*self.home_abbreviation;
        let home_score = self.home_score;
        let (away_bold, home_bold) = if self.top { ("**", "") } else { ("", "**") };
        let walkoff_bold = if walkoff { "**" } else { "" };
        let event = self.event;

        writeln!(f, "{away_abbreviation} {away_bold}{away_score}{away_bold}-{home_bold}{home_score}{home_bold} {walkoff_bold}{home_abbreviation}{walkoff_bold} ({event})")?;
        writeln!(
            f,
            "{half} **{inning}**, **{outs}** out{out_suffix}.",
            half = if self.top { "Top" } else { "Bot" },
            inning = nth(self.inning as usize),
            outs = self.outs,
            out_suffix = if self.outs == 1 { "" } else { "s" }
        )?;
        for score in &self.scores {
            writeln!(f, "{score:?}")?;
        }

        write!(f, "\n")?;

        Ok(())
    }
}

impl Display for ScoringPlayEvent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let half = if self.top { "Top" } else { "Bot" };
        let inning = nth(self.inning as usize);
        write!(f, "{half} **{inning}**:")?;
        for score in &self.scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

pub enum OffensiveSubstitution {
    PinchRunner {
        old: String,
        new: String,

        abbreviation: String,
        top: bool,
        inning: u8,
    },
    PinchHitter {
        old: String,
        new: String,

        abbreviation: String,
        top: bool,
        inning: u8,
    },
}

impl OffensiveSubstitution {
    pub fn from_play(
        play: &Value,
        parent: &Value,
        abbreviation: &str,
        id_to_object: &FxHashMap<i64, Value>,
    ) -> Result<Self> {
        let old = id_to_object
            .get(
                &play["replacedPlayer"]["id"]
                    .as_i64()
                    .context("Could not find old player in offensive substitution")?,
            )
            .context("Old Player ID wasn't in the roaster for either team")?["person"]["fullName"]
            .as_str()
            .context("Could not find old player's name in offensive substitution")?
            .to_owned();
        let new = id_to_object
            .get(
                &play["player"]["id"]
                    .as_i64()
                    .context("Could not find new player in offensive substitution")?,
            )
            .context("New Player ID wasn't in the roaster for either team")?["person"]["fullName"]
            .as_str()
            .context("Could not find new player's name in offensive substitution")?
            .to_owned();
        let abbreviation = abbreviation.to_owned();
        let top = parent["about"]["isTopInning"]
            .as_bool()
            .context("Could not tell the inning half")?;
        let inning = parent["about"]["inning"]
            .as_i64()
            .context("Could not tell the inning")? as u8;
        match play["position"]["abbreviation"]
            .as_str()
            .context("Could not get offensive substitution position abbreviation")?
        {
            "PH" => Ok(Self::PinchHitter {
                old,
                new,

                abbreviation,
                top,
                inning,
            }),
            "PR" => Ok(Self::PinchRunner {
                old,
                new,

                abbreviation,
                top,
                inning,
            }),
            _ => Err(anyhow!("Invalid abbreviation ({:?}) for offensive substitution", &play["position"]["abbreviation"])),
        }
    }
}

impl Debug for OffensiveSubstitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OffensiveSubstitution::PinchRunner {
                old,
                new,
                abbreviation,
                top,
                inning,
            } => {
                writeln!(
                    f,
                    "### [{abbreviation} Lineup Change] | {new} pinch-running for {old}"
                )?;
                writeln!(
                    f,
                    "> Inning: **{half} {n}**",
                    half = if *top { "Top" } else { "Bot" },
                    n = nth(*inning as usize)
                )?;
                writeln!(f, "")?;
                writeln!(f, "")?;
            }
            OffensiveSubstitution::PinchHitter {
                old,
                new,
                abbreviation,
                top,
                inning,
            } => {
                writeln!(
                    f,
                    "### [{abbreviation} Lineup Change] | {new} pinch-hitting for {old}"
                )?;
                writeln!(
                    f,
                    "> Inning: **{half} {n}**",
                    half = if *top { "Top" } else { "Bot" },
                    n = nth(*inning as usize)
                )?;
                writeln!(f, "")?;
                writeln!(f, "")?;
            }
        }

        Ok(())
    }
}

pub struct DefensiveSwitch {
    name: String,
    old_fielding_position: String,
    new_fielding_position: String,

    abbreviation: String,
    top: bool,
    inning: u8,
}

impl DefensiveSwitch {
    pub fn from_play(
        play: &Value,
        parent: &Value,
        abbreviation: &str,
        id_to_object: &FxHashMap<i64, Value>,
    ) -> Result<Self> {
        let name = id_to_object
            .get(
                &play["player"]["id"]
                    .as_i64()
                    .context("Could not find new player in defensive switch")?,
            )
            .context("New Player ID wasn't in the roaster for either team")?["person"]["fullName"]
            .as_str()
            .context("Could not find new player's name in defensive switch")?;
        let description = play["details"]["description"]
            .as_str()
            .context("Description must exist")?;
        Ok(Self {
            name: name.to_owned(),
            old_fielding_position: if description.contains(" remains in the game as ") {
                "PH".to_owned()
            } else {
                to_position_abbreviation(
                    description
                        .strip_prefix("Defensive switch from ")
                        .context("Defensive Switch didn't start correctly")?
                        .split_once(" to ")
                        .context("Defensive switch didn't have a `to` to split at")?
                        .0,
                )?
            },
            new_fielding_position: play["position"]["abbreviation"]
                .as_str()
                .context("Could not find player's position in defensive substitution")?
                .to_owned(),

            abbreviation: abbreviation.to_owned(),
            top: parent["about"]["isTopInning"].as_bool().context(
                "Could not find out if defensive switch was in the top or bottom of the inning",
            )?,
            inning: parent["about"]["inning"]
                .as_i64()
                .context("Could not find out defensive switch inning")? as u8,
        })
    }
}

impl Debug for DefensiveSwitch {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            name,
            old_fielding_position,
            new_fielding_position,
            abbreviation,
            top,
            inning,
        } = self;

        if old_fielding_position == "PH" || old_fielding_position == "PR" {
            writeln!(f, "### [{abbreviation} Lineup Change] | {name} remains in the game as {new_fielding_position}.")?;
        } else {
            writeln!(f, "### [{abbreviation} Lineup Change] | {name} switches from {old_fielding_position} to {new_fielding_position}.")?;
        }
        writeln!(
            f,
            "> Inning: **{half} {n}**",
            half = if *top { "Top" } else { "Bot" },
            n = nth(*inning as usize)
        )?;
        writeln!(f, "")?;
        writeln!(f, "")?;

        Ok(())
    }
}

pub struct DefensiveSubstitution {
    old: String,
    new: String,
    fielding_position: String,
    ordinal: u8,

    abbreviation: String,
    top: bool,
    inning: u8,
}

impl DefensiveSubstitution {
    pub fn from_play(
        play: &Value,
        parent: &Value,
        abbreviation: &str,
        id_to_object: &FxHashMap<i64, Value>,
    ) -> Result<Self> {
        Ok(Self {
            old: id_to_object.get(&play["replacedPlayer"]["id"].as_i64().context("Could not find old player in defensive substitution")?).context("Old Player ID wasn't in the roaster for either team")?["person"]["fullName"].as_str().context("Could not find old player's name in defensive substitution")?.to_owned(),
            new: id_to_object.get(&play["player"]["id"].as_i64().context("Could not find new player in defensive substitution")?).context("New Player ID wasn't in the roaster for either team")?["person"]["fullName"].as_str().context("Could not find new player's name in defensive substitution")?.to_owned(),
            fielding_position: play["position"]["abbreviation"].as_str().context("Could not find player's position in defensive substitution")?.to_owned(),
            ordinal: (play["battingOrder"].as_str().and_then(|s| s.parse::<usize>().ok()).context("Could not get defensive substitution's batting order")? / 100) as u8,

            abbreviation: abbreviation.to_owned(),
            top: parent["about"]["isTopInning"].as_bool().context("Could not find out if defensive substitution was in the top or bottom of the inning")?,
            inning: parent["about"]["inning"].as_i64().context("Could not find out defensive substitution inning")? as u8,
        })
    }
}

impl Debug for DefensiveSubstitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            old,
            new,
            fielding_position,
            ordinal,
            abbreviation,
            top,
            inning,
        } = self;

        writeln!(f, "### [{abbreviation} Lineup Change] | {new} replaces {old}, playing {fielding_position}, batting {n}.", n = nth(*ordinal as usize))?;
        writeln!(
            f,
            "> Inning: **{half} {n}**",
            half = if *top { "Top" } else { "Bot" },
            n = nth(*inning as usize)
        )?;
        writeln!(f, "")?;
        writeln!(f, "")?;

        Ok(())
    }
}

pub struct PitchingSubstitution {
    old: String,
    old_era: f64,
    new_id: i64,
    new: String,
    new_era: f64,
    abbreviation: String,
    innings_pitched: String,
    hits: usize,
    earned_runs: usize,
    walks: usize,
    strikeouts: usize,
    pitches: usize,
}

impl PitchingSubstitution {
    pub fn from_play(
        play: &Value,
        abbreviation: &str,
        previous_pitcher: Value,
    ) -> Result<Self> {
        let previous_pitcher_inning_stats = &previous_pitcher["people"][0]["stats"][0]["splits"].as_array().context("Could not find stats for latest game")?.last().context("Expected the player that just pitched to have stats about what they just pitched")?["stat"];
        let old = previous_pitcher["people"][0]["fullName"]
            .as_str()
            .context("Could not find old pitcher's name")?
            .to_owned();
        let new_id = play["player"]["id"]
            .as_i64()
            .context("Could not find new pitcher's name")?;
        let new_pitcher = ureq::get(&format!("https://statsapi.mlb.com/api/v1/people/{new_id}?hydrate=stats(group=[pitching],type=[gameLog])")).call()?.into_json::<Value>()?;
        let new = new_pitcher["people"][0]["fullName"]
            .as_str()
            .context("Could not find new pitcher's name")?
            .to_owned();
        let (new_era, _) = era(new_pitcher)?;
        let abbreviation = abbreviation.to_owned();
        let innings_pitched = previous_pitcher_inning_stats["inningsPitched"]
            .as_str()
            .context("Could not find pitcher's IP")?
            .to_owned();
        let hits = previous_pitcher_inning_stats["hits"]
            .as_i64()
            .context("Could not find pitcher's hits")? as usize;
        let earned_runs = previous_pitcher_inning_stats["earnedRuns"]
            .as_i64()
            .context("Could not find pitcher's earned runs")? as usize;
        let strikeouts = previous_pitcher_inning_stats["strikeOuts"]
            .as_i64()
            .context("Could not find pitcher's strikeouts")? as usize;
        let pitches = previous_pitcher_inning_stats["numberOfPitches"]
            .as_i64()
            .context("Could not find pitcher's pitch count")? as usize;
        let walks = previous_pitcher_inning_stats["baseOnBalls"]
            .as_i64()
            .context("Could not find pitcher's BB")? as usize
            + previous_pitcher_inning_stats["intentionalWalks"]
                .as_i64()
                .context("Could not find pitcher's IBB")? as usize;
        let (old_era, _) = era(previous_pitcher)?;

        Ok(Self {
            old,
            old_era,
            new_id,
            new,
            new_era,
            abbreviation,
            innings_pitched,
            hits,
            earned_runs,
            walks,
            strikeouts,
            pitches,
        })
    }

    pub fn dbg_print(&self) {
        let Self {
            old,
            old_era,
            new,
            new_era,
            new_id: _,
            abbreviation,
            innings_pitched,
            hits,
            earned_runs,
            walks,
            strikeouts,
            pitches,
        } = self;
        let handle = unsafe { GetStdHandle(-11_i32 as u32) };
        let quality_start = innings_pitched.split_once(".").expect("IP didn't have a .").0.parse::<usize>().expect("Integer IP part wasn't valid") >= 6 && *earned_runs <= 3;
        println!("### [{abbreviation} Pitching Change] | {new} ({new_era:.2} ERA) replaces {old} ({old_era:.2} ERA).");
        if quality_start {
            let _ = std::io::Write::flush(&mut stdout());
            unsafe { SetConsoleTextAttribute(handle, 0) };
            print!(":star: ");
            let _ = std::io::Write::flush(&mut stdout());
            unsafe { SetConsoleTextAttribute(handle, 7) };
        }
        print!(
            "__{last_name}'s Final Line__:",
            last_name = self.old.rsplit_once(' ').map_or(&*self.old, |(_, x)| x)
        );
        if quality_start {
            let _ = std::io::Write::flush(&mut stdout());
            unsafe { SetConsoleTextAttribute(handle, 0) };
            print!(" :star:");
            let _ = std::io::Write::flush(&mut stdout());
            unsafe { SetConsoleTextAttribute(handle, 7) };
        }
        println!("\n> **{innings_pitched}** IP | **{hits}** H | **{earned_runs}** ER | **{walks}** BB | **{strikeouts}** K");
        println!("> Pitch Count: **{pitches}**");
        println!();
        print!("");
    }
}

impl Debug for PitchingSubstitution {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let Self {
            old,
            old_era,
            new,
            new_era,
            new_id: _,
            abbreviation,
            innings_pitched,
            hits,
            earned_runs,
            walks,
            strikeouts,
            pitches,
        } = self;
        let quality_start = innings_pitched.split_once(".").expect("IP didn't have a .").0.parse::<usize>().expect("Integer IP part wasn't valid") >= 6 && *earned_runs <= 3;
        writeln!(f, "### [{abbreviation} Pitching Change] | {new} ({new_era:.2} ERA) replaces {old} ({old_era:.2} ERA).")?;
        if quality_start {
            write!(f, ":star: ")?;
        }
        write!(f,
            "__{last_name}'s Final Line__:",
            last_name = self.old.rsplit_once(' ').map_or(&*self.old, |(_, x)| x)
        )?;
        if quality_start {
            write!(f, " :star:")?;
        }
        writeln!(f, "\n> **{innings_pitched}** IP | **{hits}** H | **{earned_runs}** ER | **{walks}** BB | **{strikeouts}** K")?;
        writeln!(f, "> Pitch Count: **{pitches}**")?;
        writeln!(f)?;
        write!(f, "")
    }
}

pub struct ScoringPlay {
    inning: u8,
    top: bool,
    outs: u8,
    away_abbreviation: String,
    away_score: i64,
    home_abbreviation: String,
    home_score: i64,
    rbi: i64,
    scores: Vec<Score>,
    raw_event: String,
}

pub struct Score {
    value: String,
    scoring: bool,
}

impl Debug for Score {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.scoring {
            write!(f, "> **{}**", self.value)
        } else {
            write!(f, "> {}", self.value)
        }
    }
}

impl Display for Score {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl ScoringPlay {
    pub fn from_play(
        play: &Value,
        home_abbreviation: &str,
        away_abbreviation: &str,
        all_player_names: &[String],
    ) -> Result<Self> {
        Ok(Self {
            inning: play["about"]["inning"]
                .as_i64()
                .context("Could not find inning")? as u8,
            top: play["about"]["isTopInning"]
                .as_bool()
                .context("Could not find inning half")?,
            outs: play["count"]["outs"]
                .as_i64()
                .context("Could not find outs")? as u8,
            away_abbreviation: away_abbreviation.to_owned(),
            away_score: play["result"]["awayScore"]
                .as_i64()
                .context("Could not find away team's score")?,
            home_abbreviation: home_abbreviation.to_owned(),
            home_score: play["result"]["homeScore"]
                .as_i64()
                .context("Could not find away team's score")?,
            rbi: play["result"]["rbi"]
                .as_i64()
                .context("Could not find the RBI of the play")?,
            scores: {
                let description = play["result"]["description"]
                    .as_str()
                    .context("Play description didn't exist")?;
                let mut vec = Vec::new();
                let mut iter = description
                    .split_once(": ")
                    .map_or(description, |(_, x)| x)
                    .split("  ")
                    .map(str::trim)
                    .filter(|str| !str.is_empty());
                while let Some(value) = iter.next() {
                    let value = if all_player_names.iter().any(|name| value == *name) {
                        remap_score_event(
                            &format!(
                                "{value} {}",
                                iter.next().context("Play unexpectedly ended")?
                            ),
                            all_player_names,
                        )
                    } else {
                        remap_score_event(value, all_player_names)
                    };

                    vec.push(Score {
                        scoring: value.contains("scores.")
                            || value.contains("homers")
                            || value.contains("home run"),
                        value,
                    })
                }
                vec
            },
            raw_event: play["result"]["eventType"]
                .as_str()
                .context("Could not find event type")?
                .to_owned(),
        })
    }
}

impl Display for ScoringPlay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let half = if self.top { "Top" } else { "Bot" };
        let inning = nth(self.inning as usize);
        write!(f, "{half} **{inning}**:")?;
        for score in &self.scores {
            write!(f, " {score}")?;
        }
        Ok(())
    }
}

impl Debug for ScoringPlay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let walkoff = !self.top && self.inning >= 9 && self.home_score > self.away_score;
        let away_abbreviation = &*self.away_abbreviation;
        let away_score = self.away_score;
        let home_abbreviation = &*self.home_abbreviation;
        let home_score = self.home_score;
        let (away_bold, home_bold) = if self.top { ("**", "") } else { ("", "**") };
        let walkoff_bold = if walkoff { "**" } else { "" };
        let event = match &*self.raw_event {
            "single" => {
                if self.rbi == 1 {
                    "RBI single".to_owned()
                } else {
                    format!("{n}RBI single", n = self.rbi)
                }
            }
            "double" => {
                if self.rbi == 1 {
                    "RBI double".to_owned()
                } else {
                    format!("{n}RBI double", n = self.rbi)
                }
            }
            "triple" => {
                if self.rbi == 1 {
                    "RBI triple".to_owned()
                } else {
                    format!("{n}RBI triple", n = self.rbi)
                }
            }
            "home_run" => {
                if self.scores.iter().any(|Score { value: play, .. }| play.contains("inside-the-park")) {
                    if self.scores.len() == 1 {
                        "HR".to_owned()
                    } else {
                        format!("{}HR", self.scores.len())
                    }
                } else {
                    if self.scores.len() == 1 {
                        "**HR**".to_owned()
                    } else {
                        format!("**{}HR**", self.scores.len())
                    }
                }
            }
            "grounded_into_double_play" => {
                if self.rbi == 1 {
                    "RBI double play".to_owned()
                } else {
                    format!("{n}RBI double play", n = self.rbi)
                }
            }
            "field_out" => {
                if self.rbi == 1 {
                    "RBI out".to_owned()
                } else {
                    format!("{n}RBI out", n = self.rbi)
                }
            }
            "sac_fly" => {
                if self.rbi == 1 {
                    "RBI sacrifice fly".to_owned()
                } else {
                    format!("{n}RBI sacrifice fly", n = self.rbi)
                }
            }
            "sac_bunt" => {
                if self.rbi == 1 {
                    "RBI sacrifice bunt".to_owned()
                } else {
                    format!("{n}RBI sacrifice bunt", n = self.rbi)
                }
            }
            "fielders_choice_out" => {
                if self.rbi == 1 {
                    "RBI Fielder's choice".to_owned()
                } else {
                    format!("{n}RBI Fielder's choice", n = self.rbi)
                }
            }
            "field_error" => "Error".to_owned(),
            "intent_walk" => "Bases loaded intentional walk".to_owned(),
            "balk" => "Balk".to_owned(),
            "walk" => "Bases loaded walk".to_owned(),
            "hit_by_pitch" => "Bases loaded HBP".to_owned(),
            event => format!("{n}RBI {event}", n = self.rbi),
        };

        writeln!(f, "{away_abbreviation} {away_bold}{away_score}{away_bold}-{home_bold}{home_score}{home_bold} {walkoff_bold}{home_abbreviation}{walkoff_bold} ({event})")?;
        writeln!(
            f,
            "{half} **{inning}**, **{outs}** out{out_suffix}.",
            half = if self.top { "Top" } else { "Bot" },
            inning = nth(self.inning as usize),
            outs = self.outs,
            out_suffix = if self.outs == 1 { "" } else { "s" }
        )?;
        for score in &self.scores {
            writeln!(f, "{score:?}")?
        }

        write!(f, "\n")?;

        Ok(())
    }
}

fn to_position_abbreviation(s: &str) -> Result<String> {
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

fn nth(n: usize) -> String {
    let mut buf = String::with_capacity(n.checked_ilog10().map_or(1, |x| x + 1) as usize + 2);
    let _ = write!(&mut buf, "{n}");
    if n / 10 % 10 == 1 {
        buf.push_str("th");
    } else {
        match n % 10 {
            1 => buf.push_str("st"),
            2 => buf.push_str("nd"),
            3 => buf.push_str("rd"),
            _ => buf.push_str("th"),
        }
    }
    buf
}

fn clear_screen(height: usize) {
    let handle = unsafe { GetStdHandle(-11_i32 as u32) };
    for n in 0..height {
        unsafe {
            SetConsoleCursorPosition(handle, Coordinate { x: 0, y: n as i16 });
        }
        println!("{}", unsafe {
            core::str::from_utf8_unchecked(&[b' '; 1024])
        });
    }
}

fn get_id() -> Result<(usize, usize)> {
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
            .context("Games value didn't exist")?;
        let mut ids = Vec::with_capacity(games.len());
        let width = (games.len() + 1).checked_ilog10().map_or(1, |x| x + 1) as usize;
        println!("[{}] Please select a game ordinal to wait on for lineups (use arrows for movement and dates): \n", date.format("%m/%d/%Y"));
        for (idx, game) in games.iter().enumerate() {
            let idx = idx + 1;
            ids.push(game["gamePk"].as_i64().context("Game ID didn't exist")?);
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
                    SetConsoleTextAttribute(handle, 7);
                }
            }
            println!(
                "  {idx: >width$}. {home} vs. {away} @ {}",
                Canada__Eastern
                    .from_local_datetime(&time.naive_local())
                    .latest()
                    .context("Error converting to Canada Eastern Timezone")?
                    .format("%H:%M %Z")
            );
        }
        unsafe {
            SetConsoleTextAttribute(handle, 7);
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
                clear_screen(ids.len() + 2);
                unsafe { SetConsoleCursorPosition(handle, Coordinate { x: 0, y: 0 }); }
                return Ok((ids[idx] as usize, ids.len() + 2))
            }
            // let mut str = String::new();
            // unsafe {
            //     SetConsoleTextAttribute(GetStdHandle(-11_i32 as u32), 2);
            // }
            // stdin().lock().read_line(&mut str)?;
            // unsafe {
            //     SetConsoleTextAttribute(GetStdHandle(-11_i32 as u32), 7);
            // }
            // std::io::Write::flush(&mut stdout())?;
            // if str.starts_with('p') {
            //     date = date
            //         .pred_opt()
            //         .context("Error when getting previous date")?;
            //     clear_screen(ids.len() + 4);
            //     continue 'a;
            // } else if str.starts_with('n') {
            //     date = date.succ_opt().context("Error when getting next date")?;
            //     clear_screen(ids.len() + 4);
            //     continue 'a;
            // }
            // match str.trim_end().parse::<usize>() {
            //     Ok(idx) if (1..=games.len()).contains(&idx) => {
            //         return Ok((ids[idx - 1] as usize, ids.len() + 4))
            //     },
            //     _ => println!("Invalid game, please insert a valid ordinal or date rotation\n> "),
            // }
        }
    }
}

fn era(value: Value) -> Result<(f64, f64)> {
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

fn get_pitcher_lines(
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

pub struct RecordAgainst {
    our_abbreviation: String,
    their_abbreviation: String,
    our_record: i64,
    their_record: i64,
    scored_recently: Option<bool>, // true is us
}

impl RecordAgainst {
    pub fn new(our_abbreviation: &str, their_abbreviation: &str) -> Self {
        Self {
            our_abbreviation: our_abbreviation.to_owned(),
            their_abbreviation: their_abbreviation.to_owned(),
            our_record: 0,
            their_record: 0,
            scored_recently: None,
        }
    }

    pub fn add_older_win(&mut self) {
        self.our_record += 1;
        self.scored_recently = self.scored_recently.or(Some(true));
    }

    pub fn add_older_loss(&mut self) {
        self.their_record += 1;
        self.scored_recently = self.scored_recently.or(Some(true));
    }

    pub fn add_newer_win(&mut self) {
        self.our_record += 1;
        self.scored_recently = Some(true);
    }

    pub fn add_newer_loss(&mut self) {
        self.their_record += 1;
        self.scored_recently = Some(true);
    }
}

impl Display for RecordAgainst {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let our_abbreviation = &*self.our_abbreviation;
        let our_record = self.our_record;
        let their_abbreviation = &*self.their_abbreviation;
        let their_record = self.their_record;

        let (record_our_bold, record_them_bold) = match self.scored_recently {
            Some(true) => ("**", ""),
            Some(false) => ("", "**"),
            None => ("", ""),
        };

        let (record_bold, record_opp_bold) = match our_record.cmp(&their_record) {
            Ordering::Less => ("", "**"),
            Ordering::Equal => ("", ""),
            Ordering::Greater => ("**", ""),
        };

        write!(f, "{record_bold}{our_abbreviation}{record_bold} {record_our_bold}{our_record}{record_our_bold}-{record_them_bold}{their_record}{record_them_bold} {record_opp_bold}{their_abbreviation}{record_opp_bold}")
    }
}

pub struct Standings {
    wins: i64,
    losses: i64,
    streak: Option<(bool, NonZeroUsize)>,
}

impl Standings {
    pub fn new(wins: i64, losses: i64) -> Self {
        Self {
            wins,
            losses,
            streak: None,
        }
    }

    pub fn streak_older_win(&mut self) -> bool {
        if let Some((true, n)) = &mut self.streak {
            *n = n.saturating_add(1);
            true
        } else if self.streak.is_none() {
            self.streak = Some((true, NonZeroUsize::MIN));
            true
        } else {
            false
        }
    }

    pub fn streak_older_loss(&mut self) -> bool {
        if let Some((false, n)) = &mut self.streak {
            *n = n.saturating_add(1);
            true
        } else if self.streak.is_none() {
            self.streak = Some((false, NonZeroUsize::MIN));
            true
        } else {
            false
        }
    }

    pub fn win(&mut self) {
        self.wins += 1;
        if let Some((true, n)) = &mut self.streak {
            *n = n.saturating_add(1);
        } else {
            self.streak = Some((true, NonZeroUsize::MIN));
        }
    }

    pub fn loss(&mut self) {
        self.losses += 1;
        if let Some((false, n)) = &mut self.streak {
            *n = n.saturating_add(1);
        } else {
            self.streak = Some((false, NonZeroUsize::MIN));
        }
    }
}

impl Display for Standings {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let (wins, losses) = (self.wins, self.losses);
        if let Some((kind, streak)) = self.streak {
            if kind {
                write!(f, "**{wins}**-{losses} (__W{streak}__)")
            } else {
                write!(f, "{wins}-**{losses}** (__L{streak}__)")
            }
        } else {
            write!(f, "0-0 (__N/A__)")
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
        .rev()
        .flat_map(|game| game["games"].as_array())
        .flat_map(|x| x.iter())
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

fn title(home: bool, home_full: &str, away_full: &str) -> String {
    if home {
        format!("{home_full} vs. {away_full}")
    } else {
        format!("{away_full} @ {home_full}")
    }
}

fn real_abbreviation(parent: &Value) -> Result<String> {
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

fn hide(s: &str) -> String {
    s.chars().map(|x| if x.is_ascii_whitespace() { "" } else { r"\_" }).collect::<String>()
}

fn write_last_lineup_underscored(out: &mut String, previous_loadout: &Value) -> Result<()> {
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

fn lineup(root: &Value, prev: &Value) -> Result<String> {
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
        let prev_batting_order = prev_player
            .and_then(|player| player["battingOrder"].as_str())
            .and_then(|x| x.parse::<i64>().ok());
        let (prev_position, position) = (
            prev_player.and_then(|x| x["allPositions"][0]["abbreviation"].as_str()),
            player["position"]["abbreviation"]
                .as_str()
                .context("Hitter's first position didn't exist")?,
        );
        let avg = player["seasonStats"]["batting"]["avg"]
            .as_str()
            .context("Hitter's avg didn't exist")?;
        let slg = player["seasonStats"]["batting"]["slg"]
            .as_str()
            .context("Hitter's slg didn't exist")?;
        let stats = format!("({avg} *|* {slg})");
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
            format!("{name_and_index} {position} {stats}"),
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
