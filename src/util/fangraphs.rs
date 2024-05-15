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
    let raw = ureq::post("https://www.fangraphs.com/guts.aspx?type=cn").call().expect("Got wOBA constants successfully").into_string().expect("Response was a valid string");
    let current_year = Local::now().year();
    let start = raw.find(&format!(r#"<td class="grid_line_regular">{current_year}"#)).expect("Expected index from guts board");
    let end = raw.find(&format!(r#"<td class="grid_line_regular">{last_year}"#, last_year = current_year - 1)).expect("Expected index from guts board");
    let year = raw.split_at(start).1.split_at(end - start).0.split_at(r#"<td class="grid_line_regular">XXXX</td>"#.len()).1;
    let last_td = year.rfind(r#"</td>"#).expect("Expected at least one </td> tag");
    let year = year.split_at(last_td + r#"</td>"#.len()).0;
    let year = year.replace("</td><td class=\"grid_line_regular\" align=\"right\">", "</td>\n<td class=\"grid_line_regular\" align=\"right\">");
    let stats = year.split("\n").map(|line| line.strip_prefix(r#"<td class="grid_line_regular" align="right">"#).unwrap_or(line).strip_suffix(r#"</td>"#).unwrap_or(line).to_string()).collect::<Vec<_>>();

    WobaConstants {
        lgwOBA: stats[0].parse::<f64>().unwrap(),
        wOBAScale: stats[1].parse::<f64>().unwrap(),
        wBB: stats[2].parse::<f64>().unwrap(),
        wHBP: stats[3].parse::<f64>().unwrap(),
        w1B: stats[4].parse::<f64>().unwrap(),
        w2B: stats[5].parse::<f64>().unwrap(),
        w3B: stats[6].parse::<f64>().unwrap(),
        wHR: stats[7].parse::<f64>().unwrap(),
        runSB: stats[8].parse::<f64>().unwrap(),
        runCS: stats[9].parse::<f64>().unwrap(),
        lgRPA: stats[10].parse::<f64>().unwrap(),
    }
}

fn get_ballpark_adjustments() -> FxHashMap<String, f64> {
    let raw = ureq::post("https://www.fangraphs.com/guts.aspx?type=pf&season=2023&teamid=0&sort=2,d").call().expect("Got ballpark factors successfully").into_string().expect("Response was a valid string");
    let start = raw.find(r#"</thead><tbody>"#).expect("Expected index from guts board");
    let end = raw.find(r#"</table><div id="GutsBoard1_dg1_SharedCalendarContainer" style="display:none;">"#).expect("Expected index from guts board");
    let data = raw.split_at(start + r#"</thead><tbody>"#.len()).1.split_at(end - start - r#"</thead><tbody>"#.len()).0.trim_end();
    let data = data.strip_suffix(r#"</tbody>"#).unwrap_or(data).trim_end();
    let data = data.strip_suffix(r#"</tr>"#).unwrap_or(data).trim_end();

    data.split("</tr>").map(|line| {
        let line = line.trim();
        let team_name = line.split_once(r#"</td><td class="grid_line_regular">"#).expect("Could not find ballpark adjustment").1.split_once(r#"</td><td class="grid_line_regular rgSorted" align="right""#).expect("Could not find ballpark adjustment").0;
        let adjustment = line.split_once(r##"</td><td class="grid_line_regular rgSorted" align="right" bgcolor="#E5E5E5">"##).expect("Could find ballpark adjustment").1.split_once(r#"</td>"#).expect("Could find ballpark adjustment").0.parse::<i64>().expect("Valid i64 for ballpark adjustment") as f64 / 100.0;
        (team_name.to_string(), adjustment)
    }).collect::<FxHashMap<String, f64>>()
}

pub static WOBA_CONSTANTS: LazyLock<WobaConstants> = LazyLock::new(get_woba_constants);
pub static BALLPARK_ADJUSTMENTS: LazyLock<FxHashMap<String, f64>> = LazyLock::new(get_ballpark_adjustments);