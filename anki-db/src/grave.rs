// Code in this file is based on or derived from the Anki project.
// You can find the original code at https://github.com/ankitects/anki.

use rusqlite::params;
use rusqlite::Transaction;
use num_enum::TryFromPrimitive;

use crate::Error;

#[derive(TryFromPrimitive)]
#[repr(u8)]
pub(crate) enum GraveKind {
    Card,
    Note,
    Deck,
}

pub(crate) fn add_grave(
    trans: &Transaction,
    oid: i64,
    usn: i32,
    kind: GraveKind,
) -> Result<(), Error> {
    trans
        .prepare_cached(include_str!("../sql/add_grave.sql"))?
        .execute(params![usn, oid, kind as u8])?;
    Ok(())
}
