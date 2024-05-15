use std::sync::LazyLock;
use chrono::{Datelike, Local};

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
    pub fn calculate_wRCp(self, bb: usize, hbp: usize, singles: usize, doubles: usize, triples: usize, home_runs: usize, stolen_bases: usize, caught_stealings: usize, pa: usize, ibb: usize) -> i64 {
        let Self { lgwOBA, wOBAScale, lgRPA, .. } = self;
        let wOBA = self.calculate_wOBA(bb, hbp, singles, doubles, triples, home_runs, stolen_bases, caught_stealings, pa - ibb);
        (100.0 * (((wOBA - lgwOBA) / wOBAScale + lgRPA) / lgRPA)).round() as i64
    }
}

pub fn get_woba_constants() -> WobaConstants {
    let raw = ureq::post("https://www.fangraphs.com/guts.aspx?type=cn").call().expect("Got wOBA constants successfully").into_string().expect("Response was a string");
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

pub static WOBA_CONSTANTS: LazyLock<WobaConstants> = LazyLock::new(get_woba_constants);