mod card;
mod deck;
mod deckconfig;
mod error;
mod grave;
mod note;
mod notetype;
mod text;

pub use deck::AnkiDeck;
pub use error::Error;
pub use note::Note;
pub use notetype::Notetype;

use rusqlite::Connection;

pub fn check_db_compatibility(conn: &mut Connection) -> Result<bool, Error> {
    let trans = conn.transaction()?;
    let ver: i32 = trans.query_row("SELECT ver FROM col", [], |row| row.get(0))?;
    Ok(ver == 18) // compatibility is only ensured for db scheme 18
}
