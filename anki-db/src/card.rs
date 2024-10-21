// Code in this file is based on or derived from the Anki project.
// You can find the original code at https://github.com/ankitects/anki.

use core::result;
use num_enum::TryFromPrimitive;
use rusqlite::{
    params,
    types::{FromSql, FromSqlError, ValueRef},
    Row, Transaction,
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::error::Error;
use crate::grave::{add_grave, GraveKind};

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub(crate) cid: Option<i64>,
    pub(crate) nid: Option<i64>,
    pub(crate) did: i64,
    pub(crate) template_idx: u16,
    pub(crate) mtime: i64,
    pub(crate) usn: i32,
    pub(crate) ctype: CardType,
    pub(crate) queue: CardQueue,
    pub(crate) due: i32,
    pub(crate) interval: u32,
    pub(crate) ease_factor: u16,
    pub(crate) reps: u32,
    pub(crate) lapses: u32,
    pub(crate) remaining_steps: u32,
    pub(crate) original_due: i32,
    pub(crate) original_deck_id: i64,
    pub(crate) flags: u8,
    pub(crate) original_position: Option<u32>,
    pub(crate) memory_state: Option<FsrsMemoryState>,
    pub(crate) desired_retention: Option<f32>,
    pub(crate) custom_data: String,
}

impl Default for Card {
    fn default() -> Self {
        Self {
            cid: None,
            nid: None,
            did: 0,
            template_idx: 0,
            mtime: 0,
            usn: -1,
            ctype: CardType::New,
            queue: CardQueue::New,
            due: 1,
            interval: 0,
            ease_factor: 0,
            reps: 0,
            lapses: 0,
            remaining_steps: 0,
            original_due: 0,
            original_deck_id: 0, // must be zero
            flags: 0,
            original_position: None,
            memory_state: None,
            desired_retention: None,
            custom_data: String::new(),
        }
    }
}

impl Card {
    pub(crate) fn from_row(row: &Row) -> Result<Self, Error> {
        let data: CardData = row.get(17)?;
        Ok(Self {
            cid: row.get(0)?,
            nid: row.get(1)?,
            did: row.get(2)?,
            template_idx: row.get(3)?,
            mtime: row.get(4)?,
            usn: row.get(5)?,
            ctype: row.get(6)?,
            queue: row.get(7)?,
            due: row.get(8).ok().unwrap_or_default(),
            interval: row.get(9)?,
            ease_factor: row.get(10)?,
            reps: row.get(11)?,
            lapses: row.get(12)?,
            remaining_steps: row.get(13)?,
            original_due: row.get(14).ok().unwrap_or_default(),
            original_deck_id: row.get(15)?,
            flags: row.get(16)?,
            original_position: data.original_position,
            memory_state: data.memory_state(),
            desired_retention: data.fsrs_desired_retention,
            custom_data: data.custom_data,
        })
    }

    pub(crate) fn write_to_db(&mut self, trans: &Transaction) -> Result<i64, Error> {
        let mut stmt = trans.prepare_cached(include_str!("../sql/add_card.sql"))?;

        stmt.execute(params![
            1,
            self.nid,
            self.did,
            self.template_idx,
            self.mtime,
            self.usn,
            self.ctype as u8,
            self.queue as i8,
            self.due,
            self.interval,
            self.ease_factor,
            self.reps,
            self.lapses,
            self.remaining_steps,
            self.original_due,
            self.original_deck_id,
            self.flags,
            CardData::from_card(&self).convert_to_json()?,
        ])?;

        self.cid = Some(trans.last_insert_rowid());

        Ok(self.cid.unwrap())
    }

    pub fn delete(self, trans: &Transaction) -> Result<(), Error> {
        // do nothing if not written to db
        if let Some(cid) = self.cid {
            add_grave(trans, cid, self.usn, GraveKind::Card)?;
            trans
                .prepare_cached("delete from cards where id = ?")?
                .execute([cid])?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FsrsMemoryState {
    // the expected memory stability, in days
    pub stability: f32,
    // a number in the range 1.0-10.0
    pub difficulty: f32,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, PartialEq, Eq, TryFromPrimitive, Clone, Copy)]
#[repr(u8)]
pub enum CardType {
    New = 0,
    Learn = 1,
    Review = 2,
    Relearn = 3,
}

impl FromSql for CardType {
    fn column_result(value: ValueRef<'_>) -> result::Result<Self, FromSqlError> {
        if let ValueRef::Integer(i) = value {
            Ok(Self::try_from(i as u8).map_err(|_| FromSqlError::InvalidType)?)
        } else {
            Err(FromSqlError::InvalidType)
        }
    }
}

#[derive(Serialize_repr, Deserialize_repr, Debug, PartialEq, Eq, TryFromPrimitive, Clone, Copy)]
#[repr(i8)]
pub enum CardQueue {
    // due is the order cards are shown in
    New = 0,
    // due is a unix timestamp
    Learn = 1,
    // due is days since creation date
    Review = 2,
    DayLearn = 3,
    // due is a unix timestamp, preview cards only placed here when failed.
    PreviewRepeat = 4,
    // cards are not due in these states
    Suspended = -1,
    SchedBuried = -2,
    UserBuried = -3,
}

impl FromSql for CardQueue {
    fn column_result(value: ValueRef<'_>) -> result::Result<Self, FromSqlError> {
        if let ValueRef::Integer(i) = value {
            Ok(Self::try_from(i as i8).map_err(|_| FromSqlError::InvalidType)?)
        } else {
            Err(FromSqlError::InvalidType)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct CardData {
    #[serde(
        rename = "pos",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "default_on_invalid"
    )]
    pub(crate) original_position: Option<u32>,
    #[serde(
        rename = "s",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "default_on_invalid"
    )]
    pub(crate) fsrs_stability: Option<f32>,
    #[serde(
        rename = "d",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "default_on_invalid"
    )]
    pub(crate) fsrs_difficulty: Option<f32>,
    #[serde(
        rename = "dr",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "default_on_invalid"
    )]
    pub(crate) fsrs_desired_retention: Option<f32>,

    // a string representation of a JSON object storing optional data associated with the card, so v3 custom scheduling code can persist state
    #[serde(default, rename = "cd", skip_serializing_if = "meta_is_empty")]
    pub(crate) custom_data: String,
}

impl CardData {
    pub(crate) fn from_card(card: &Card) -> Self {
        Self {
            original_position: card.original_position,
            fsrs_stability: card.memory_state.as_ref().map(|m| m.stability),
            fsrs_difficulty: card.memory_state.as_ref().map(|m| m.difficulty),
            fsrs_desired_retention: card.desired_retention,
            custom_data: card.custom_data.clone(),
        }
    }

    pub(crate) fn memory_state(&self) -> Option<FsrsMemoryState> {
        if let Some(stability) = self.fsrs_stability {
            if let Some(difficulty) = self.fsrs_difficulty {
                return Some(FsrsMemoryState {
                    stability,
                    difficulty,
                });
            }
        }
        None
    }

    pub(crate) fn convert_to_json(&mut self) -> Result<String, Error> {
        if let Some(v) = &mut self.fsrs_stability {
            round_to_places(v, 2)
        }
        if let Some(v) = &mut self.fsrs_difficulty {
            round_to_places(v, 3)
        }
        if let Some(v) = &mut self.fsrs_desired_retention {
            round_to_places(v, 2)
        }
        Ok(serde_json::to_string(&self)?)
    }
}

fn round_to_places(value: &mut f32, decimal_places: u32) {
    let factor = 10_f32.powi(decimal_places as i32);
    *value = (*value * factor).round() / factor;
}

impl FromSql for CardData {
    // infallible; invalid/missing data results in the default value
    fn column_result(value: ValueRef<'_>) -> std::result::Result<Self, FromSqlError> {
        if let ValueRef::Text(s) = value {
            Ok(serde_json::from_slice(s).unwrap_or_default())
        } else {
            Ok(Self::default())
        }
    }
}

pub(crate) fn default_on_invalid<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let v: Value = Deserialize::deserialize(deserializer)?;
    Ok(T::deserialize(v).unwrap_or_default())
}

fn meta_is_empty(s: &str) -> bool {
    matches!(s, "" | "{}")
}
