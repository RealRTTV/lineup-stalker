use core::cmp::Ordering;
use core::fmt::{Display, Formatter};

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
        self.scored_recently = Some(false);
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