#![allow(non_snake_case)]

use anyhow::Result;
use mlb_api::stats::raw::hitting;
use mlb_api::stats::wrappers::{WithNone, WithPlayer};
use mlb_api::stats::TwoDecimalPlaceRateStat;
use std::fmt::Display;

#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum HittingStat {
    AVG,
    SLG,
    OBP,
    OPS,
    BABIP,
    BB,
    K,
    ISO,
    BBK,
    wOBA,
    wRCp,
}

impl HittingStat {
    pub const MAX_NAME_WIDTH: usize = 5;

    pub fn prev(self) -> Self {
        match self {
            Self::AVG => Self::wRCp,
            Self::SLG => Self::AVG,
            Self::OBP => Self::SLG,
            Self::OPS => Self::OBP,
            Self::BABIP => Self::OPS,
            Self::BB => Self::BABIP,
            Self::K => Self::BB,
            Self::ISO => Self::K,
            Self::BBK => Self::ISO,
            Self::wOBA => Self::BBK,
            Self::wRCp => Self::wOBA,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::AVG => Self::SLG,
            Self::SLG => Self::OBP,
            Self::OBP => Self::OPS,
            Self::OPS => Self::BABIP,
            Self::BABIP => Self::BB,
            Self::BB => Self::K,
            Self::K => Self::ISO,
            Self::ISO => Self::BBK,
            Self::BBK => Self::wOBA,
            Self::wOBA => Self::wRCp,
            Self::wRCp => Self::AVG,
        }
    }

    pub async fn get(self, stats: &WithNone<hitting::__BoxscoreStatsData>, sabermetrics_stats: impl AsyncFnOnce() -> Result<WithPlayer<hitting::__SabermetricsStatsData>>) -> String {
        use mlb_api::stats::derived::*;

        match self {
            Self::AVG => avg(stats.hits, stats.at_bats).to_string(),
            Self::SLG => slg(stats.total_bases, stats.at_bats).to_string(),
            Self::OBP => obp(stats.hits, stats.base_on_balls, stats.intentional_walks, stats.hit_by_pitch, stats.at_bats, stats.sac_bunts, stats.sac_flies).to_string(),
            Self::OPS => ops(Ok(obp(stats.hits, stats.base_on_balls, stats.intentional_walks, stats.hit_by_pitch, stats.at_bats, stats.sac_bunts, stats.sac_flies)), Ok(slg(stats.total_bases, stats.at_bats))).to_string(),
            Self::BABIP => babip(stats.hits, stats.home_runs, stats.at_bats, stats.strikeouts, stats.sac_flies).to_string(),
            Self::BB => bb_pct(stats.base_on_balls, stats.plate_appearances).to_string(),
            Self::K => k_pct(stats.strikeouts, stats.plate_appearances).to_string(),
            Self::ISO => iso(extra_bases(stats.doubles, stats.triples, stats.home_runs), stats.at_bats).to_string(),
            Self::BBK => TwoDecimalPlaceRateStat::new(strikeout_to_walk_ratio(stats.strikeouts, stats.base_on_balls).recip()).to_string(),
            Self::wOBA => sabermetrics_stats().await.ok().and_then(|stats| stats.wOBA.ok()).unwrap_or_default().to_string(),
            Self::wRCp => sabermetrics_stats().await.ok().and_then(|stats| stats.wRCp.ok()).unwrap_or_default().to_string(),
        }
    }
}

impl Display for HittingStat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match *self {
            Self::AVG => "AVG",
            Self::SLG => "SLG",
            Self::OBP => "OBP",
            Self::OPS => "OPS",
            Self::BABIP => "BABIP",
            Self::BB => "BB",
            Self::K => "K",
            Self::ISO => "ISO",
            Self::BBK => "BB/K",
            Self::wOBA => "wOBA",
            Self::wRCp => "wRC+",
        })
    }
}
