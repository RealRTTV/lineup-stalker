use std::sync::LazyLock;
use chrono::{Datelike, Local};
use fxhash::FxHashMap;

#[allow(non_snake_case)]
#[derive(Copy, Clone, Debug)]
pub struct WobaConstants {
    lgwOBA: f64,
    wOBAScale: f64,
    wBB: f64,
    wHBP: f64,
    w1B: f64,
    w2B: f64,
    w3B: f64,
    wHR: f64,
    runSB: f64,
    runCS: f64,
    lgRPA: f64,
}

#[allow(non_snake_case)]
impl WobaConstants {
    pub fn calculate_wOBA(self, bb: usize, hbp: usize, singles: usize, doubles: usize, triples: usize, home_runs: usize, stolen_bases: usize, caught_stealings: usize, pa_minus_ibb: usize) -> f64 {
        let Self { wBB, wHBP, w1B, w2B, w3B, wHR, runSB, runCS, .. } = self;
        let wBB = bb as f64 * wBB;
        let wHBP = hbp as f64 * wHBP;
        let w1B = singles as f64 * w1B;
        let w2B = doubles as f64 * w2B;
        let w3B = triples as f64 * w3B;
        let wHR = home_runs as f64 * wHR;
        let runSB = stolen_bases as f64 * runSB;
        let runCS = caught_stealings as f64 * runCS;

        (wBB + wHBP + w1B + w2B + w3B + wHR + runSB + runCS) / pa_minus_ibb as f64
    }

    #[inline]
    pub fn calculate_wRCp(self, bb: usize, hbp: usize, singles: usize, doubles: usize, triples: usize, home_runs: usize, stolen_bases: usize, caught_stealings: usize, pa: usize, ibb: usize, team: &str) -> i64 {
        let Self { lgwOBA, wOBAScale, lgRPA, .. } = self;
        let wOBA = self.calculate_wOBA(bb, hbp, singles, doubles, triples, home_runs, stolen_bases, caught_stealings, pa - ibb);
        (100.0 * (((wOBA - lgwOBA) / wOBAScale + lgRPA * (2.0 - BALLPARK_ADJUSTMENTS.get(team).expect("Expected team to have adjustments"))) / lgRPA)).round() as i64
    }
}

fn get_woba_constants() -> WobaConstants {
    let raw_owned = ureq::get("https://www.fangraphs.com/guts.aspx?type=cn").call().expect("Got wOBA constants successfully").into_string().expect("Response was a valid string");
    let mut current_year = Local::now().year();
    let start = 'a: {
        while current_year > 0 {
            if let Some(start) = raw_owned.find(&format!(r#"="Season" class="align-right fixed">{current_year}</td>"#)) {
                break 'a start;
            }
            current_year -= 1;
        }
        panic!("Failed to get WOBA constants, no years found.")
    };
    let (_, raw) = raw_owned.split_at(start);
    let (raw, _) = raw.split_once(&r#"</tr>"#).expect("Expected index from guts board");
    let map = parse_table_row_all_nums(raw);

    WobaConstants {
        lgwOBA: map["wOBA"],
        wOBAScale: map["wOBAScale"],
        wBB: map["wBB"],
        wHBP: map["wHBP"],
        w1B: map["w1B"],
        w2B: map["w2B"],
        w3B: map["w3B"],
        wHR: map["wHR"],
        runSB: map["runSB"],
        runCS: map["runCS"],
        lgRPA: map["R/PA"],
    }
}

fn get_ballpark_adjustments() -> FxHashMap<String, f64> {
    let raw_owned = ureq::get("https://www.fangraphs.com/guts.aspx?type=pf&season=2023&teamid=0&sort=2,d").call().expect("Got ballpark factors successfully").into_string().expect("Response was a valid string");
    let (_, raw) = raw_owned.split_once(r#"</thead><tbody>"#).expect("Incorrect specification");
    let (raw, _) = raw.split_once(r#"</tbody>"#).expect("Incorrect specification");
    parse_whole_table(raw).into_iter().map(|map| {
        let team_name = map["Team"].as_ref().unwrap_err().to_owned();
        let &adjustment = map["1yr"].as_ref().unwrap();
        (team_name, adjustment)
    }).collect()
}

fn parse_whole_table(raw: &str) -> Vec<FxHashMap<String, Result<f64, String>>> {
    let raw = raw.trim_end().strip_suffix(r#"</tr>"#).map_or(raw, str::trim_end).trim_start();
    raw.split("</tr>").map(parse_table_row).collect::<Vec<FxHashMap<String, Result<f64, String>>>>()
}

#[must_use]
fn parse_table_row(raw: &str) -> FxHashMap<String, Result<f64, String>> {
    let raw = raw.trim_end().strip_suffix(r#"</td>"#).map_or(raw, str::trim_end).trim_start();
    raw.split("</td>").map(|line| {
        let line = line.trim();
        let line = line.replace(r#" class="align-right""#, "").replace(r#" class="align-right fixed""#, "").replace(r#" class="align-left fixed""#, "").replace(r#" class="align-left""#, "");
        let (_, line) = line.rsplit_once(r#"=""#).expect("Incorrect specification");
        let (key, value) = line.split_once(r#"">"#).expect("Incorrect specification");
        (key.to_owned(), value.parse().ok().ok_or_else(|| value.to_owned()))
    }).collect()
}

#[allow(unused)]
fn parse_whole_table_all_nums(raw: &str) -> Vec<FxHashMap<String, f64>> {
    let raw = raw.trim_end().strip_suffix(r#"</tr>"#).map_or(raw, str::trim_end).trim_start();
    raw.split("</tr>").map(parse_table_row_all_nums).collect::<Vec<FxHashMap<String, f64>>>()
}

#[must_use]
fn parse_table_row_all_nums(raw: &str) -> FxHashMap<String, f64> {
    let raw = raw.trim_end().strip_suffix(r#"</td>"#).map_or(raw, str::trim_end).trim_start();
    raw.split("</td>").map(|line| {
        let line = line.trim();
        let line = line.replace(r#" class="align-right""#, "").replace(r#" class="align-right fixed""#, "").replace(r#" class="align-left fixed""#, "").replace(r#" class="align-left""#, "");
        let (_, line) = line.rsplit_once(r#"=""#).expect("Incorrect specification");
        let (key, value) = line.split_once(r#"">"#).expect("Incorrect specification");
        (key.to_owned(), value.parse().expect("Expected all numbers"))
    }).collect()
}

pub static WOBA_CONSTANTS: LazyLock<WobaConstants> = LazyLock::new(get_woba_constants);
pub static BALLPARK_ADJUSTMENTS: LazyLock<FxHashMap<String, f64>> = LazyLock::new(get_ballpark_adjustments);