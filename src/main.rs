use core::ffi::c_void;
use std::convert::identity;
use std::io::{stderr, stdout};
use std::ops::{ControlFlow, Deref};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::posts::final_card::FinalCard;
use crate::posts::lineup::Lineup;
use crate::posts::pitching_line::PitcherFinalLine;
use crate::posts::scoring_play::ScoringPlay;
use crate::posts::scoring_play_event::ScoringPlayEvent;
use crate::posts::Post;
use crate::posts::components::decisions::Decisions;
use crate::util::fangraphs::{BALLPARK_ADJUSTMENTS, WOBA_CONSTANTS};
use crate::util::ffi::{self, ConsoleCursorInfo};
use crate::posts::components::line_score::LineScore;
use crate::posts::components::next_game::NextGame;
use crate::posts::components::pitching::PitcherLineupEntry;
use crate::posts::components::record_against::RecordAgainst;
use crate::posts::components::standings::Standings;
use crate::util::stat::HittingStat;
use crate::util::statsapi::{get_last_lineup_underscores, modify_abbreviation, pitching_stats, BoldingDisplayKind, Score};
use crate::posts::components::team_stats_log::TeamStatsLog;
use crate::util::{clear_screen, get_team_color_escape, statsapi};
use anyhow::{anyhow, ensure, Context, Result};
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use chrono_tz::Tz;
use chrono_tz::Tz::America__Toronto;
use fxhash::FxHashSet;
use mlb_api::game::{GameId, LiveFeedRequest, LiveFeedResponse, PlayEvent, PlayStream, PlayStreamEvent};
use mlb_api::request::RequestURLBuilderExt;
use mlb_api::schedule::{ScheduleGame, ScheduleRequest, ScheduleResponse};
use mlb_api::sport::SportId;
use mlb_api::{venue_hydrations, HomeAway, TeamSide};
use mlb_api::meta::EventType;
use mlb_api::person::PersonId;
use mlb_api::venue::VenuesRequest;
use serde_json::Value;

pub const TIMEZONE: Tz = America__Toronto;

pub mod util;
pub mod posts;

// todo: reimplement cancelled listener
// fn create_cancelled_listener() -> Arc<AtomicBool> {
//     let cancelled = Arc::new(AtomicBool::new(false));
//     let cancelled_clone = Arc::clone(&cancelled);
//     tokio::task::spawn(move || {
//         loop {
//             let key = ffi::read_char();
//             if key == 0x08 {
//                 cancelled_clone.store(true, Ordering::Relaxed);
//                 break
//             }
//         }
//     });
//     cancelled
// }

#[tokio::main]
async fn main() {
    loop {
        clear_screen(128);
        ffi::set_cursor(0, 0);
        if let Err(e) = main0().await {
            eprintln!("Error while stalking lineup: {e}");
        }
        eprint!("\nPress any key to continue... ");
        let _ = std::io::Write::flush(&mut stderr());
        let _ = ffi::read_char();
    }
}

async fn main0() -> Result<()> {
    pub async fn await_filled_batting_order(mut live_feed: Result<LiveFeedResponse, GameId>, cheering_for: TeamSide) -> Result<LiveFeedResponse> {
        let mut dots = 0;
        ffi::set_cursor_visible(false);

        let mut live_feed = match live_feed {
            Ok(game) => game,
            Err(id) => LiveFeedRequest::builder().id(id).build_and_get().await?,
        };

        loop {
            if live_feed.live.boxscore.teams.choose(cheering_for).batting_order.is_empty() {
                print!("\rLoading{: <pad$}", ".".repeat(dots + 1), pad = 3 - dots);
                ffi::flush();
                dots = (dots + 1) % 3;
                live_feed = LiveFeedRequest::builder().id(live_feed.id).build_and_get().await?;
                tokio::time::sleep(Duration::new(live_feed.meta.recommended_poll_rate as _, 0)).await;
            } else {
                println!("         ");
                break;
            }
        }
        ffi::set_cursor_visible(true);
        Ok(live_feed)
    }

    ffi::set_cursor_visible(false);
    let (game_id, cheering_for, stats) = get_id()?;
    ffi::set_cursor(0, 0);
    let mut live_feed: LiveFeedResponse = LiveFeedRequest::builder().id(game_id).build_and_get()?;
    let (lineup, next_game, (home_pitcher_id, away_pitcher_id)) = lines(&live_feed, cheering_for, game_id, stats)?;
    let mut post = Post::Lineup(lineup);
    post.send_with_settings(true, true, true)?;
    live_feed = await_filled_batting_order(Ok(live_feed), cheering_for).await?;
    ffi::set_cursor(0, 0);
    if let Post::Lineup(inner) = &mut post {
        let lineup = statsapi::lineup(&live_feed.live.boxscore.teams.choose(cheering_for), stats, statsapi::should_show_stats(live_feed.data.game_type), live_feed.data.season)?;
        inner.update_lineup(lineup);
        post.send()?;
    }
    let Post::Lineup(Lineup { record, standings, .. }) = post else { return Err(anyhow!("Post was not a lineup??")) };
    posts_loop(
        live_feed,
        cheering_for,
        standings,
        record,
        next_game,
        home_pitcher_id,
        away_pitcher_id,
    )?;
    Ok(())
}

async fn get_id() -> Result<(GameId, TeamSide, [HittingStat; 2])> {
    const PREFERRED_TEAM_NAMES: &[&str] = &[
        "Toronto Blue Jays"
    ];

    let mut idx = 0_usize;
    let mut date = Local::now().date_naive();
    'a: loop {
        ffi::set_cursor(0, 0);
        let games = ScheduleRequest::<()>::builder()
            .sport_id(SportId::MLB)
            .date(date)
            .build_and_get()?
            .dates
            .into_iter()
            .next()
            .map_or(vec![], |date| date.games);
        let idx_width = (games.len() + 1).checked_ilog10().map_or(1, |x| x + 1) as usize;
        println!("[{}] Please select a game ordinal to wait on for lineups (use arrows for movement and dates): \n", date.format("%A, %B %e %Y"));
        for (idx, game) in games.iter().enumerate() {
            print_game(game, idx, idx_width, "0", "38;5;10", false)?;
        }
        ffi::set_cursor(0, 2);
        print!("> ");
        std::io::Write::flush(&mut stdout())?;
        ffi::set_cursor(0, 2);
        loop {
            match ffi::read_char() {
                first_char @ (0x33..=0x39 | 0x30) => {
                    let month = if first_char == 0x30 { 10 } else { 3 + (first_char - 0x33) };
                    idx = 0;
                    date = date.with_day(1).context("Error when setting day to 1")?.with_month(month).context("Error when setting month")?;
                    clear_screen(games.len() + 2);
                    ffi::set_cursor(0, 0);
                    continue 'a;
                }
                0xE0 => {
                    match ffi::read_char() {
                        0x48 => {
                            ffi::set_cursor(0, idx + 2);
                            print!("  ");
                            ffi::flush();
                            idx = idx.saturating_sub(1);
                            ffi::set_cursor(0, idx + 2);
                            print!("> ");
                            ffi::flush();
                        },
                        0x50 => {
                            ffi::set_cursor(0, idx + 2);
                            print!("  ");
                            ffi::flush();
                            idx = (idx + 1).min(games.len() - 1);
                            ffi::set_cursor(0, idx + 2);
                            print!("> ");
                            ffi::flush();
                        },
                        0x4B => {
                            idx = 0;
                            date = date.pred_opt().context("Error when getting previous date")?;
                            clear_screen(games.len() + 2);
                            ffi::set_cursor(0, 0);
                            continue 'a;
                        },
                        0x4D => {
                            idx = 0;
                            date = date.succ_opt().context("Error when getting next date")?;
                            clear_screen(games.len() + 2);
                            ffi::set_cursor(0, 0);
                            continue 'a;
                        },
                        _ => {},
                    }
                },
                0x0D if !games.is_empty() => {
                    ffi::set_cursor(0, 2);
                    for (current_idx, game) in games.iter().enumerate() {
                        if current_idx == idx {
                            print_game(game, current_idx, idx_width, "0", "38;5;10", false)?;
                        } else {
                            print_game(game, current_idx, idx_width, "90", "38;5;9", true)?;
                        }
                        thread::sleep(Duration::from_millis(35.saturating_sub(current_idx as u64)));
                    }
                    let game = &games[idx];
                    Ok((game.game_id, select_team_side(game, &date, idx, games.len()), get_hitting_stats(&date)))
                }
                _ => {},
            }
        }
    }

    fn print_game(game: &ScheduleGame<()>, idx: usize, idx_width: usize, default_color_escape: &str, preferred_team_color_escape: &str, always_use_default_color: bool) -> Result<()> {
        let num = idx + 1;
        let home_name = game.teams.home.team.full_name.as_str();
        let away_name = &game.teams.away.team.full_name.as_str();
        let (color_escape, home_color_escape, away_color_escape) = if PREFERRED_TEAM_NAMES.contains(&home_name) || PREFERRED_TEAM_NAMES.contains(&away_name) {
            (preferred_team_color_escape, preferred_team_color_escape, preferred_team_color_escape)
        } else {
            (default_color_escape, get_team_color_escape(home_name), get_team_color_escape(away_name))
        };
        let timestamp = TIMEZONE.from_utc_datetime(&game.game_date).format("%H:%M %Z");
        if always_use_default_color {
            println!("\x1B[{color_escape}m  {num: >idx_width$}. {home_name} vs. {away_name} @ {timestamp}\x1B[{color_escape}m");
        } else {
            println!("\x1B[{color_escape}m  {num: >idx_width$}. \x1B[{home_color_escape}m{home_name}\x1B[{color_escape}m vs. \x1B[{away_color_escape}m{away_name}\x1B[{color_escape}m @ {timestamp}");
        }
        ffi::set_text_attribute(7);
        Ok(())
    }

    fn select_team_side(game: &ScheduleGame<()>, date: &NaiveDate, game_idx: usize, num_games: usize) -> TeamSide {
        ffi::set_cursor(0, 0);
        println!("[{}] Please select the home team or away team (use arrows for switching):                                \n", date.format("%A, %B %e %Y"));
        ffi::set_cursor(0, game_idx + 2);
        let home_name = game.teams.home.team.full_name.as_str();
        let away_name = game.teams.away.team.full_name.as_str();
        let time = TIMEZONE.from_utc_datetime(&game.game_date);
        let timestamp = time.format("%H:%M %Z");
        ffi::set_text_attribute(7);
        println!(
            "> \x1B[{home_color_escape}m{home_name}\x1B[0m vs. \x1B[{away_color_escape}m{away_name}\x1B[0m @ {timestamp}                                ",
            home_color_escape = get_team_color_escape(home_name),
            away_color_escape = get_team_color_escape(away_name),
        );
        print!("  {home_underline}                                                                \r", home_underline = "^".repeat(home_name.len()));
        ffi::flush();
        let mut selected_team_side = TeamSide::Home;
        loop {
            match ffi::read_char() {
                0xE0 => if let 0x4B | 0x4D = ffi::read_char() {
                    selected_team_side = !selected_team_side;
                    let (home_symbol, away_symbol) = HomeAway::new(("^", " "), (" ", "^")).choose(selected_team_side);
                    print!("  {home_underline}     {away_underline}                                                                \r", home_underline = home_symbol.repeat(home_name.len()), away_underline = away_symbol.repeat(away_name.len()));
                    ffi::flush();
                },
                0x0D => {
                    clear_screen(num_games + 2);
                    ffi::set_cursor(0, 0);
                    return selected_team_side;
                },
                _ => {},
            }
        }
    }

    fn get_hitting_stats(date: &NaiveDate) -> [HittingStat; 2] {
        println!("[{}] Please select hitting stats (use arrows):                                \n", date.format("%A, %B %e %Y"));
        let mut stats = [HittingStat::AVG, HittingStat::wRCp];
        let mut selected_stat_idx = 0_usize;
        loop {
            ffi::set_cursor(0, 2);
            {
                ffi::set_text_attribute(8);
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
                ffi::set_text_attribute(8);
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
                ffi::set_text_attribute(8);
                print!("  ");
                for (idx, stat) in stats.iter().enumerate() {
                    print!("{next: ^width$}", next = stat.next().to_string(), width = HittingStat::MAX_NAME_WIDTH);
                    if idx + 1 < stats.len() {
                        print!(" | ");
                    }
                }
                println!();
                ffi::set_text_attribute(7);
            }
            match ffi::read_char() {
                0xE0 => {
                    match ffi::read_char() {
                        0x4B | 0x4D => selected_stat_idx = 1 - selected_stat_idx,
                        0x48 => stats[selected_stat_idx] = stats[selected_stat_idx].prev(),
                        0x50 => stats[selected_stat_idx] = stats[selected_stat_idx].next(),
                        _ => {},
                    }
                },
                0x0D => {
                    clear_screen(5);
                    return stats
                },
                _ => {},
            }
        }
    }
}

async fn posts_loop(
    live_feed: LiveFeedResponse,
    cheering_for: TeamSide,
    mut standings: Standings,
    mut record: RecordAgainst,
    next_game: Option<NextGame>,
    home_starter_id: PersonId,
    away_starter_id: PersonId,
) -> Result<()> {
    let game_id = live_feed.id;
    let game_type = live_feed.data.game_type;
    let all_player_names = live_feed.data.players
        .values()
        .map(|player| player.full_name.as_str())
        .collect::<Vec<&str>>();
    let mut scoring_plays = Vec::new();
    let mut previous_play_plus_play_event_len = 0;

    let HomeAway { home: home_starter, away: away_starter } = HomeAway::new(home_starter_id, away_starter_id).map(|id| live_feed.data.players.get(&id).expect("SP was not in game").full_name.clone());

    PlayStream::with_presupplied_feed::<anyhow::Error, _>(live_feed, async |event, meta, data, linescore, boxscore| {
        match event {
            PlayStreamEvent::EndPlay(play) => {
                if play.about.is_scoring_play == Some(true) {
                    let scoring_play = ScoringPlay::from_play(
                        &play,
                        &data.teams.home.name.abbreviation,
                        &data.teams.away.name.abbreviation,
                        &all_player_names,
                    )?;
                    scoring_plays.push(scoring_play.as_one_liner());
                    Post::ScoringPlay(scoring_play).send()?;
                }
            }
            PlayStreamEvent::PlayEvent(play_event, play) => {
                match play_event {
                    PlayEvent::Action { details, common, .. } => {
                        match details.event {
                            EventType::PitchingSubstitution if details.description.contains(&home_starter) || details.description.contains(&away_starter) => {
                                let id = if details.description.contains(home_starter) {
                                    home_starter_id
                                } else {
                                    away_starter_id
                                };
                                let pitching_substitution = PitcherFinalLine::from_play(boxscore.find_player_with_game_data(id).context("Pitcher did not play in the game?")?)?;
                                Post::PitchingSubstitution(pitching_substitution).send()?;
                            },
                            EventType::PassedBall | EventType::WildPitch if details.is_scoring_play => {
                                let wild_pitch = ScoringPlayEvent::from_play(
                                    (details, common),
                                    play,
                                    &data.teams.home.name.abbreviation,
                                    &data.teams.away.name.abbreviation,
                                    &all_player_names,
                                    EventType::WildPitch,
                                )?;
                                scoring_plays.push(wild_pitch.as_one_liner());
                                Post::WildPitch(wild_pitch).send()?;
                            },
                            EventType::StolenBaseHome => {
                                let stolen_home = ScoringPlayEvent::from_play(
                                    (details, common),
                                    play,
                                    &data.teams.home.name.abbreviation,
                                    &data.teams.away.name.abbreviation,
                                    &all_player_names,
                                    EventType::StolenBase,
                                )?;
                                scoring_plays.push(stolen_home.as_one_liner());
                                Post::StolenHome(stolen_home).send()?;
                            }
                            _ => {},
                        }
                    }
                    _ => {},
                }
            }
            PlayStreamEvent::GameEnd(decisions, _, _, _stat_leaders) => {
                let last_inning = linescore.innings.last().context("You gotta have at least one inning if the game is over")?.clone();
                let walkoff = linescore.rhe_totals.home.runs > linescore.rhe_totals.away.runs
                    && linescore.rhe_totals.home.runs
                    - last_inning.inning_record.home.runs <= linescore.rhe_totals.away.runs;
                let linescore_component = LineScore::new(linescore)?;

                if (linescore.rhe_totals.away.runs > linescore.rhe_totals.home.runs) ^ (cheering_for == TeamSide::Home) {
                    standings.win();
                    record.win();
                } else {
                    standings.loss();
                    record.loss();
                }

                let pitching_masterpiece = TeamStatsLog::generate_masterpiece(&home, &away, innings.len(), &home.abbreviation).unwrap_or(String::new()) + &TeamStatsLog::generate_masterpiece(&away, &home, innings.len(), &away.abbreviation).unwrap_or(String::new());
                let decisions = Decisions::new(decisions, boxscore);

                Post::FinalCard(FinalCard::new(Score::from_stats_log(&home, &away, num_innings as u8, false, BoldingDisplayKind::WinningTeam, if walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None }), (game_type != "P").then_some(standings), if game_type == "P" { "Series Against" } else { "Record Against" }, record, next_game, pitching_masterpiece, line_score, scoring_plays, decisions)).send()?;
                
                return Ok(())
            }
            _ => {},
        }

        Ok(ControlFlow::Continue(()))
    }).await?;
}

fn lines(
    live_feed: &LiveFeedResponse,
    cheering_for: TeamSide,
    game_pk: GameId,
    hitting_stats: [HittingStat; 2],
) -> Result<(Lineup, Option<NextGame>, (PersonId, PersonId))> {
    venue_hydrations! {
        struct VenueWithTimezone {
            timezone
        }
    }

    let HomeAway { home: (home_full, home_abbreviation), away: (away_full, away_abbreviation) } = live_feed.data.teams.as_ref().map(|team| (team.full_name.as_str(), team.name.abbreviation.as_str()));

    let datetime = TIMEZONE.from_utc_datetime(&*live_feed.data.datetime);
    let local_datetime = VenuesRequest::<VenueWithTimezone>::builder().venue_ids(vec![live_feed.data.venue.id]).build_and_get()?.venues[0].extras.timezone;
    let time = if datetime.naive_local() == local_datetime.naive_local() {
        format!("{}", datetime.format("%H:%M %Z"))
    } else {
        format!("{} / {}", datetime.format("%H:%M %Z"), local_datetime.format("%H:%M %Z"))
    };

    thread::scope(|s| {
        let pitcher_future = s.spawn(|| get_pitcher_lines(live_feed, &home_abbreviation, &away_abbreviation));

        let (previous_game, standings, record, next_game) = response_parsed_values(&live_feed, cheering_for, game_pk)?;

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

            let previous_team_lineup = previous_game["liveData"]["boxscore"]["teams"][if cheering_for {
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
            hitting_stats,
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
    response: &live_feed::GameLiveFeed,
    home_abbreviation: &str,
    away_abbreviation: &str,
) -> Result<((PitcherLineupEntry, PlayerId), (PitcherLineupEntry, PlayerId))> {
    let Person { full_name: home_pitcher, id: home_pitcher_id } = &response.game_data.probable_pitchers.home;
    let Person { full_name: away_pitcher, id: away_pitcher_id } = &response.game_data.probable_pitchers.away;

    let (home_era, home_ip, home_hand) = pitching_stats(get(&format!("https://statsapi.mlb.com/api/v1/people/{home_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?)?;
    let (away_era, away_ip, away_hand) = pitching_stats(get(&format!("https://statsapi.mlb.com/api/v1/people/{away_pitcher_id}?hydrate=stats(group=[pitching],type=[gameLog])"))?)?;

    let away_pitcher_stats = PitcherLineupEntry::new(away_pitcher.to_owned(), away_abbreviation.to_owned(), away_hand, away_era, away_ip);
    let home_pitcher_stats = PitcherLineupEntry::new(home_pitcher.to_owned(), home_abbreviation.to_owned(), home_hand, home_era, home_ip);

    Ok((
        (away_pitcher_stats, *away_pitcher_id),
        (home_pitcher_stats, *home_pitcher_id),
    ))
}
