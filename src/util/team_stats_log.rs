pub struct TeamStatsLog {
    pub walks: usize,
    pub hits: usize,
    pub errors: usize,
    pub runs: usize,
    pub strikeouts: usize,
    pub pitches_thrown: usize,
    pub pitchers: Vec<String>,
    pub abbreviation: String,
}

impl TeamStatsLog {
    pub fn new(starting_pitcher_name: String, abbreviation: String) -> Self {
        Self {
            walks: 0,
            hits: 0,
            errors: 0,
            runs: 0,
            strikeouts: 0,
            pitches_thrown: 0,
            pitchers: vec![starting_pitcher_name],
            abbreviation,
        }
    }

    pub fn walk(&mut self) {
        self.walks += 1;
    }

    pub fn strikeout(&mut self) {
        self.strikeouts += 1;
    }

    pub fn pitch_thrown(&mut self) {
        self.pitches_thrown += 1;
    }

    pub fn change_pitcher(&mut self, reliever: String) {
        self.pitchers.push(reliever);
    }

    pub fn add_runs(&mut self, runs: usize) {
        self.runs += runs;
    }

    pub fn add_hits(&mut self, hits: usize) {
        self.hits += hits;
    }

    pub fn add_errors(&mut self, errors: usize) {
        self.errors += errors;
    }

    pub fn opponent_game_score(&self, innings: usize) -> i32 {
        50 + 3 * innings as i32 + 2 * innings.saturating_sub(4) as i32 + self.strikeouts as i32 - 2 * self.hits as i32 - 4 * self.runs as i32 - self.walks as i32
    }

    pub fn generate_masterpiece(&self, opponent: &Self, innings: usize, abbreviation: &str) -> Option<String> {
        let masterpiece_kind = if opponent.hits == 0 {
            if opponent.walks == 0 && self.errors == 0 {
                Some("Perfect Game")
            } else {
                Some("No-Hitter")
            }
        } else if self.is_complete_game() {
            if self.is_shutout() {
                Some("Complete Game Shutout")
            } else {
                Some("Complete Game")
            }
        } else {
            None
        };
        if let Some(masterpiece_kind) = masterpiece_kind {
            let game_score = opponent.opponent_game_score(innings);

            Some(format!(
                "### {abbreviation} {combined}{masterpiece_kind}{maddux_suffix}\n:star: __{pitcher_names}'s Final Line__ :star:\n> **{innings}.0** IP | **{hits}** H | **{runs}** ER | **{walks}** BB | {strikeout_surroundings}**{strikeouts}** K{strikeout_surroundings} | **{game_score}** GS\n> Pitch Count: {maddux_left}**{pitches_thrown}**{maddux_right}\n",
                combined = if !self.is_complete_game() { "Combined " } else { "" },
                maddux_suffix = if self.is_maddux() { " Maddux" } else { "" },
                pitcher_names = self.get_pitcher_names(),
                hits = opponent.hits,
                runs = opponent.runs,
                walks = opponent.walks,
                strikeouts = opponent.strikeouts,
                pitches_thrown = self.pitches_thrown,
                strikeout_surroundings = if opponent.strikeouts >= 12 { "__" } else { "" },
                maddux_left = if self.is_maddux() { "__*" } else { "" },
                maddux_right = if self.is_maddux() { "*__" } else { "" }
            ))
        } else {
            None
        }
    }

    pub fn get_pitcher_names(&self) -> String {
        self.pitchers.join("/")
    }

    pub fn is_complete_game(&self) -> bool {
        self.pitchers.len() <= 1
    }

    pub fn is_shutout(&self) -> bool {
        self.runs == 0
    }

    pub fn is_complete_game_shutout(&self) -> bool {
        self.is_complete_game() && self.is_shutout()
    }

    pub fn is_maddux(&self) -> bool {
        self.pitches_thrown < 100 && self.is_complete_game_shutout()
    }
}


