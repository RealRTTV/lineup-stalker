use core::fmt::Write;
use crate::set_cursor;

pub mod ffi;
pub mod statsapi;
pub mod record_against;
pub mod standings;
pub mod stat;
pub mod next_game;
pub mod decisions;
pub mod fangraphs;
pub mod team_stats_log;
pub mod pitching;
pub mod hitting;
pub mod line_score;

pub fn nth(n: usize) -> String {
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

pub fn clear_screen(height: usize) {
    for n in 0..height {
        set_cursor(0, n);
        println!("{}", unsafe {
            core::str::from_utf8_unchecked(&[b' '; 1024])
        });
    }
}

pub fn hide(s: &str) -> String {
    s.chars().map(|x| if x.is_ascii_whitespace() { " " } else { r"\_" }).collect::<String>()
}

pub fn get_team_color_escape(team: &str) -> &'static str {
    match team {
        "Arizona Diamondbacks" => "38;2;167;25;48",
        "Atlanta braves" => "38;2;206;17;65",
        "Baltimore Orioles" => "38;2;223;70;1",
        "Boston Red Sox" => "38;2;189;48;57",
        "Chicago Cubs" => "38;2;14;51;134",
        "Chicago White Sox" => "38;2;39;37;31",
        "Cincinnati Reds" => "38;2;198;1;31",
        "Cleveland Guardians" => "38;2;0;56;93",
        "Colorado Rockies" => "38;2;51;51;102",
        "Detroit Tigers" => "38;2;12;35;64",
        "Houston Astros" => "38;2;0;45;98",
        "Kansas City Royals" => "38;2;0;70;135",
        "Los Angeles Angels" => "38;2;186;0;33",
        "Los Angeles Dodgers" => "38;2;0;90;156",
        "Miami Marlins" => "38;2;0;163;224",
        "Milwaukee Brewers" => "38;2;255;197;47",
        "Minnesota Twins" => "38;2;0;43;92",
        "New York Mets" => "38;2;0;45;114",
        "New York Yankees" => "38;2;196;206;211",
        "Oakland Athletics" => "38;2;0;56;49",
        "Philadelphia Phillies" => "38;2;232;24;40",
        "Pittsburgh Pirates" => "38;2;39;37;31",
        "San Diego Padres" => "38;2;47;36;29",
        "San Francisco Giants" => "38;2;253;90;30",
        "Seattle Mariners" => "38;2;12;44;86",
        "St. Louis Cardinals" => "38;2;196;30;58",
        "Tampa Bay Rays" => "38;2;9;44;92",
        "Texas Rangers" => "38;2;0;50;120",
        "Toronto Blue Jays" => "38;2;19;74;142",
        "Washington Nationals" => "38;2;171;0;3",
        _ => "0",
    }
}