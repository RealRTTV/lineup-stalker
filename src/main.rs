use std::io::{stderr, stdout};
use std::ops::ControlFlow;
use std::thread;
use std::time::Duration;

use components::decisions::Decisions;
use components::linescore::LineScore;
use components::next_game::NextGame;
use components::pitching::PitcherLineupEntry;
use components::record_against::RecordAgainst;
use components::standings::Standings;
use crate::posts::final_card::FinalCard;
use crate::posts::lineup::Lineup;
use crate::posts::pitching_line::PitcherFinalLine;
use crate::posts::scoring_play::ScoringPlay;
use crate::posts::scoring_play_event::ScoringPlayEvent;
use crate::posts::Post;
use crate::util::ffi::{self};
use crate::util::stat::HittingStat;
use crate::util::statsapi::{get_last_lineup_underscores, modify_abbreviation, BoldingDisplayKind, Score};
use crate::util::{clear_screen, get_team_color_escape, statsapi};
use anyhow::{bail, Context, Result};
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use chrono_tz::Tz;
use chrono_tz::Tz::America__Toronto;
use fxhash::FxHashSet;
use mlb_api::game::{GameId, LiveFeedRequest, LiveFeedResponse, PlayEvent, PlayStream, PlayStreamEvent};
use mlb_api::meta::{EventType, GameType};
use mlb_api::request::RequestURLBuilderExt;
use mlb_api::schedule::{ScheduleGame, ScheduleRequest};
use mlb_api::sport::SportId;
use mlb_api::venue::VenuesRequest;
use mlb_api::{venue_hydrations, HomeAway, TeamSide};
use mlb_api::person::PersonId;
use mlb_api::stats::derived::era;
use crate::components::pitching_masterpiece::PitchingMasterpiece;

pub const TIMEZONE: Tz = America__Toronto;

pub mod util;
pub mod posts;
pub mod components;
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
            if live_feed.live.boxscore.teams.as_ref().choose(cheering_for).batting_order.is_empty() {
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
    let (mut lineup_post, next_game) = lines(&live_feed, cheering_for, stats).await?;
    lineup_post.send_with_settings(true, true, true)?;
    live_feed = await_filled_batting_order(Ok(live_feed), cheering_for).await?;
    ffi::set_cursor(0, 0);
    let lineup = statsapi::lineup(&live_feed.live.boxscore.teams.choose(cheering_for), stats, statsapi::should_show_stats(live_feed.data.game_type), live_feed.data.season)?;
    lineup_post.update_lineup(lineup);
    lineup_post.send()?;
    posts_loop(
        live_feed,
        cheering_for,
        lineup_post.standings,
        lineup_post.record,
        next_game,
    ).await?;
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
) -> Result<()> {
    let our_abbreviation = modify_abbreviation(&live_feed.data.teams.as_ref().choose(cheering_for).name);
    let mut scoring_plays = String::new();
    let HomeAway { home: home_starter_id, away: away_starter_id } = live_feed.data.probable_pitchers.as_ref().map(|person| person.id);

    PlayStream::with_presupplied_feed::<anyhow::Error, _>(live_feed, async |event, meta, data, linescore, boxscore| {
        match event {
            PlayStreamEvent::EndPlay(play) => {
                if play.about.is_scoring_play == Some(true) {
                    let scoring_play = ScoringPlay::from_play(
                        &play,
                        &data.teams.home.name.abbreviation,
                        &data.teams.away.name.abbreviation,
                        &live_feed.data.players,
                    )?;
                    writeln!(&mut scoring_plays, "{}", scoring_play.as_one_liner())?;
                    scoring_play.send()?;
                }
            }
            PlayStreamEvent::PlayEvent(play_event, play) => {
                match play_event {
                    PlayEvent::Action { details, common, .. } => {
                        match details.event {
                            EventType::PitchingSubstitution if data.game_type != GameType::SpringTraining => {
                                let new_pitcher = common.player.unwrap_or(PersonId::new(0));
                                let new_pitcher_team = boxscore.teams.as_ref()
                                    .map(|team| team.pitchers.iter().nth(2).is_some_and(|second_pitcher| second_pitcher == new_pitcher));
                                let id = if new_pitcher_team.home {
                                    home_starter_id
                                } else {
                                    away_starter_id
                                };
                                let final_line = PitcherFinalLine::from_play(boxscore.find_player_with_game_data(id).context("Pitcher did not play in the game?")?);
                                final_line.send()?;
                            },
                            event @ (EventType::PassedBall | EventType::WildPitch | EventType::StolenBaseHome) if details.is_scoring_play => {
                                let simplified_event_type = match event {
                                    EventType::PassedBall | EventType::WildPitch => EventType::WildPitch,
                                    EventType::StolenBaseHome => EventType::StolenBase,
                                    _ => bail!("Unknown scoring play event type"),
                                };
                                let scoring_play_event = ScoringPlayEvent::from_play(
                                    (details, common),
                                    play,
                                    &data.teams.home.name.abbreviation,
                                    &data.teams.away.name.abbreviation,
                                    &live_feed.data.players,
                                    simplified_event_type,
                                )?;
                                writeln!(&mut scoring_plays, "{}", scoring_play_event.as_one_liner())?;
                                scoring_play_event.send()?
                            },
                            _ => {},
                        }
                    }
                    _ => {},
                }
            }
            PlayStreamEvent::GameEnd(decisions, _, _, _stat_leaders) => {
                let last_inning_runs = linescore.innings.last().map(|inning| inning.inning_record.map(|rhe| rhe.runs)).unwrap_or_default();
                let is_walkoff = linescore.rhe_totals.home.runs > linescore.rhe_totals.away.runs && linescore.rhe_totals.home.runs - last_inning_runs.home <= linescore.rhe_totals.away.runs;

                if (linescore.rhe_totals.away.runs > linescore.rhe_totals.home.runs) ^ (cheering_for == TeamSide::Home) {
                    standings.win();
                    record.win();
                } else {
                    standings.loss();
                    record.loss();
                }

                FinalCard {
                    score: Score::new(
                        &data.teams.away.name.abbreviation,
                        &linescore.rhe_totals.away.runs,
                        &data.teams.home.name.abbreviation,
                        &linescore.rhe_totals.home.runs,
                        linescore.innings.len() as u8,
                        TeamSide::Home, // does not matter
                        BoldingDisplayKind::WinningTeam,
                        if is_walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None },
                    ),
                    standings: (!data.game_type.is_postseason()).then_some(standings),
                    record_text: if data.game_type.is_postseason() { "Series Against" } else { "Record Against" },
                    record,
                    next_game,
                    pitching_masterpiece: PitchingMasterpiece::new(boxscore.teams.as_ref().choose(cheering_for), &our_abbreviation),
                    linescore: LineScore::new(linescore, data.teams.as_ref())?,
                    scoring_plays: scoring_plays.trim_end().to_owned(),
                    decisions: Decisions::new(decisions, boxscore),
                }.send()?;
                
                return Ok(())
            }
            _ => {},
        }

        Ok(ControlFlow::Continue(()))
    }).await?;
}

async fn lines(
    live_feed: &LiveFeedResponse,
    cheering_for: TeamSide,
    hitting_stats: [HittingStat; 2],
) -> Result<(Lineup, Option<NextGame>)> {
    venue_hydrations! {
        struct VenueWithTimezone {
            timezone
        }
    }

    let our_id = live_feed.data.teams.as_ref().choose(cheering_for).id;
    let HomeAway { home: (home_full, home_abbreviation), away: (away_full, away_abbreviation) } = live_feed.data.teams.as_ref().map(|team| (team.full_name.as_str(), team.name.abbreviation.as_str()));

    let datetime = TIMEZONE.from_utc_datetime(&*live_feed.data.datetime);
    let local_datetime = VenuesRequest::<VenueWithTimezone>::builder().venue_ids(vec![live_feed.data.venue.id]).build_and_get()?.venues[0].extras.timezone;
    let time = if datetime.naive_local() == local_datetime.naive_local() {
        format!("{}", datetime.format("%H:%M %Z"))
    } else {
        format!("{} / {}", datetime.format("%H:%M %Z"), local_datetime.format("%H:%M %Z"))
    };

    let pitchers = get_pitcher_lines(live_feed, HomeAway::new(&home_abbreviation, &away_abbreviation));

    let (previous_game_id, standings, record, next_game) = response_parsed_values(&live_feed, cheering_for).await?;
    let (previous, previous_game_team_with_game_data) = if let Some(game_id) = previous_game_id {
        let live_feed = LiveFeedRequest::builder().id(game_id).build_and_get().await?;
        let cheering_for = if live_feed.data.teams.home.id == our_id { TeamSide::Home } else { TeamSide::Away };
        let HomeAway { home: home_runs, away: away_runs } = live_feed.live.linescore.rhe_totals.map(|totals| totals.runs);
        let HomeAway { home: home_abbreviation, away: away_abbreviation } = live_feed.data.teams.as_ref().map(|team| modify_abbreviation(&team.name.abbreviation));
        let last_inning_runs = live_feed.live.linescore.innings.last().map(|inning| inning.inning_record.map(|rhe| rhe.runs)).unwrap_or_default();
        let innings = live_feed.live.linescore.innings.len();
        let is_walkoff = innings >= 9 && home_runs > away_runs && home_runs - last_inning_runs.home <= away_runs;
        let team_with_game_data = live_feed.live.boxscore.teams.choose(cheering_for);
        (Some(Score::new(away_abbreviation, away_runs, home_abbreviation, home_runs, innings as u8, false, BoldingDisplayKind::WinningTeam, if is_walkoff { BoldingDisplayKind::WinningTeam } else { BoldingDisplayKind::None })), Some(team_with_game_data))
    } else {
        (None, None)
    };

    let title = match cheering_for {
        TeamSide::Home => format!("{home_full} vs. {away_full}"),
        TeamSide::Away => format!("{away_full} @ {home_full}"),
    };

    Ok((Lineup::new(
        datetime,
        title,
        time,
        previous,
        record,
        standings,
        pitchers,
        hitting_stats,
        get_last_lineup_underscores(previous_game_team_with_game_data),
    ), next_game))
}

async fn response_parsed_values(live_feed: &LiveFeedResponse, cheering_for: TeamSide) -> Result<(Option<GameId>, Standings, RecordAgainst, Option<NextGame>)> {
    let our_team = live_feed.data.teams.as_ref().choose(cheering_for);
    let their_team = live_feed.data.teams.as_ref().choose(!cheering_for);
    let game_type = live_feed.data.game_type;
    let start_time = live_feed.data.datetime.datetime;

    let mut all_games = ScheduleRequest::<()>::builder()
        .sport_id(SportId::MLB)
        .date_range(NaiveDate::from_ymd_opt(start_time.year(), 1, 1).context("Valid date")?..=NaiveDate::from_ymd_opt(start_time.year(), 12, 31).context("Valid date")?)
        // .game_type(game_type)
        .team_id(our_team.id)
        .build_and_get().await?
        .dates.into_iter().flat_map(|date| date.games);

    let mut record = RecordAgainst::new(&our_team.name.abbreviation, &their_team.name.abbreviation);
    let mut standings = Standings::new();
    let mut games_played = FxHashSet::<GameId>::with_capacity_and_hasher(162, Default::default());

    let mut previous_game_id = None;

    for game in all_games.by_ref().take_while(|game| game.game_date < start_time && game.status.abstract_game_code.is_finished()) {
        if !games_played.insert(game.game_id) {
            continue
        }

        let Some(home_score) = game.teams.home.score else { continue };
        let Some(away_score) = game.teams.away.score else { continue };

        let is_matchup = game.teams.either(|team| team.id == their_team.id);
        if (home_score.runs_scored > away_score.runs_scored) ^ (our_team.id == game.teams.home.team.id) {
            if is_matchup { record.loss() }
            standings.loss();
        } else {
            if is_matchup { record.win() }
            standings.win();
        }

        previous_game_id = Some(game.game_id);
    }

    let next_game = if let Some(game) = all_games.skip_while(|game| game.game_date <= start_time).next() {
        NextGame::new(game, our_team.id).await?
    } else {
        None
    };

    Ok((previous_game_id, standings, record, next_game))
}

pub fn get_pitcher_lines(live_feed: &LiveFeedResponse, abbreviation: HomeAway<&str>) -> HomeAway<PitcherLineupEntry> {
    live_feed.data.probable_pitchers.as_ref().map(|person| person.id).combine(abbreviation, |a, b| (a, b)).combine(live_feed.live.boxscore.teams.as_ref(), |(pitcher, abbreviation), team| {
        let pitcher = &team.players[&pitcher];
        let person = &live_feed.data.players[&pitcher];
        PitcherLineupEntry::new(person.full_name.clone(), person.id, abbreviation.to_owned(), person.pitch_hand, era(pitcher.season_stats.pitching.earned_runs, pitcher.season_stats.pitching.innings_pitched), pitcher.season_stats.pitching.innings_pitched.unwrap_or_default())
    })
}
