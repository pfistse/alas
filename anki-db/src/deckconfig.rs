// Code in this file is based on or derived from the Anki project.
// You can find the original code at https://github.com/ankitects/anki.

use prost::Message;
use rusqlite::{params, Transaction};

pub use anki_proto::deck_config::deck_config::{
    config::{
        AnswerAction as AnswerActionProto, LeechAction as LeechActionProto,
        NewCardGatherPriority as NewCardGatherPriorityProto,
        NewCardInsertOrder as NewCardInsertOrderProto, NewCardSortOrder as NewCardSortOrderProto,
        ReviewCardOrder as ReviewCardOrderProto, ReviewMix,
    },
    Config as DeckConfigInnerProto,
};

use crate::Error;

#[derive(Debug, PartialEq, Clone)]
pub struct DeckConfig {
    pub dcid: Option<i64>,
    pub name: String,
    pub mtime: i64,
    pub usn: i64,
    pub inner: DeckConfigInnerProto,
}

const DEFAULT_DECK_CONFIG_INNER: DeckConfigInnerProto = DeckConfigInnerProto {
    learn_steps: Vec::new(),
    relearn_steps: Vec::new(),
    new_per_day: 20,
    reviews_per_day: 200,
    new_per_day_minimum: 0,
    initial_ease: 2.5,
    easy_multiplier: 1.3,
    hard_multiplier: 1.2,
    lapse_multiplier: 0.0,
    interval_multiplier: 1.0,
    maximum_review_interval: 36_500,
    minimum_lapse_interval: 1,
    graduating_interval_good: 1,
    graduating_interval_easy: 4,
    new_card_insert_order: NewCardInsertOrderProto::Due as i32,
    new_card_gather_priority: NewCardGatherPriorityProto::Deck as i32,
    new_card_sort_order: NewCardSortOrderProto::Template as i32,
    review_order: ReviewCardOrderProto::Day as i32,
    new_mix: ReviewMix::MixWithReviews as i32,
    interday_learning_mix: ReviewMix::MixWithReviews as i32,
    leech_action: LeechActionProto::TagOnly as i32,
    leech_threshold: 8,
    disable_autoplay: false,
    cap_answer_time_to_secs: 1, // NOT DEFAULT: answer time is not important
    show_timer: false,
    stop_timer_on_answer: false,
    seconds_to_show_question: 0.0,
    seconds_to_show_answer: 0.0,
    answer_action: AnswerActionProto::BuryCard as i32,
    wait_for_audio: true,
    skip_question_when_replaying_answer: false,
    bury_new: false,
    bury_reviews: false,
    bury_interday_learning: false,
    fsrs_weights: vec![],
    desired_retention: 0.9,
    other: Vec::new(),
    historical_retention: 0.9,
    weight_search: String::new(),
    ignore_revlogs_before_date: String::new(),
};

impl Default for DeckConfig {
    fn default() -> Self {
        DeckConfig {
            dcid: None,
            name: "".to_string(),
            mtime: 0,
            usn: -1,
            inner: DeckConfigInnerProto {
                learn_steps: vec![1.0, 10.0],
                relearn_steps: vec![10.0],
                ..DEFAULT_DECK_CONFIG_INNER
            },
        }
    }
}

impl DeckConfig {
    pub fn write_to_db(&mut self, trans: &Transaction) -> Result<i64, Error> {
        let mut conf_bytes = vec![];
        self.inner.encode(&mut conf_bytes)?;

        trans
            .prepare_cached(include_str!("../sql/add_deckconfig.sql"))?
            .execute(params![
                self.dcid, self.name, self.mtime, self.usn, conf_bytes,
            ])?;

        self.dcid = Some(trans.last_insert_rowid());

        Ok(self.dcid.unwrap())
    }

    pub fn load(dcid: i64, trans: &Transaction) -> Result<Option<Self>, Error> {
        let mut stmt = trans.prepare_cached(include_str!("../sql/get_deckconfig.sql"))?;
        let mut rows = stmt.query(params![dcid])?;

        // deckconfig with id found?
        let row = match rows.next()? {
            Some(row) => row,
            None => return Ok(None),
        };

        let config = DeckConfigInnerProto::decode(row.get_ref_unwrap(4).as_blob()?)?;

        Ok(Some(DeckConfig {
            dcid: row.get(0)?,
            name: row.get(1)?,
            mtime: row.get(2)?,
            usn: row.get(3)?,
            inner: config,
        }))
    }

    pub fn get_id(&self) -> Option<i64> {
        self.dcid
    }
}
