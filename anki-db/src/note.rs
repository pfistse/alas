// Code in this file is based on or derived from the Anki project.
// You can find the original code at https://github.com/ankitects/anki.

use rusqlite::{params, Transaction};
use sha1::{Digest, Sha1};

use crate::grave::{add_grave, GraveKind};
use crate::text::strip_html_preserving_media_filenames;
use crate::{card::Card, deck::AnkiDeck, notetype::Notetype, Error};

#[derive(Debug, PartialEq, Clone)]
pub struct Note {
    nid: Option<i64>,
    guid: String,
    ntid: i64,
    mtime: i64,
    usn: i32,
    tags: Vec<String>,
    fields: Vec<String>,
    // sort_field: Option<String>,
    // checksum: Option<u32>,
    cards: Option<Vec<Card>>,
}

impl Note {
    pub fn new(ntid: i64) -> Self {
        Self {
            nid: None,
            guid: base91_u64(),
            ntid,
            mtime: 0,
            usn: -1,
            tags: vec![],
            fields: vec![],
            cards: Some(Vec::new()),
        }
    }

    pub fn load_without_cards(trans: &Transaction, nid: i64) -> Result<Option<Self>, Error> {
        let mut stmt = trans.prepare_cached(include_str!("../sql/get_note.sql"))?;
        let mut rows = stmt.query(params![nid])?;

        // note with id found?
        let row = match rows.next()? {
            Some(row) => row,
            None => return Ok(None),
        };

        let tags = split_tags(row.get_ref(5)?.as_str()?)
            .map(Into::into)
            .collect();

        let fields = split_fields(row.get_ref(6)?.as_str()?);

        let note = Note {
            nid: Some(row.get(0)?),
            guid: row.get(1)?,
            ntid: row.get(2)?,
            mtime: row.get(3)?,
            usn: row.get(4)?,
            tags,
            fields,
            cards: None,
        };

        Ok(Some(note))
    }

    pub fn load_cards(&mut self, trans: &Transaction) -> Result<(), Error> {
        assert!(
            self.nid.is_some(),
            "note must be written to db before loading cards"
        );

        self.cards = Some(
            trans
                .prepare_cached(concat!(
                    include_str!("../sql/get_card.sql"),
                    " where nid = ?"
                ))?
                .query_and_then([self.nid.unwrap()], |r| Card::from_row(r))?
                .collect::<Result<Vec<Card>, Error>>()?,
        );

        Ok(())
    }

    pub fn with_fields(mut self, fields: Vec<String>) -> Self {
        self.fields = fields;
        self
    }

    pub fn with_field_entry(mut self, entry: &str) -> Self {
        self.fields.push(entry.to_string());
        self
    }

    pub fn generate_cards(mut self, nt: &Notetype, deck: &AnkiDeck) -> Self {
        let did = deck.get_id().expect("deck must be written to database");

        self.cards = Some(
            (0..nt.num_templates())
                .map(|t| Card {
                    nid: self.nid,
                    did,
                    template_idx: t.try_into().unwrap(),
                    ..Default::default()
                })
                .collect(),
        );

        self
    }

    pub fn write_to_db(&mut self, trans: &Transaction) -> Result<i64, Error> {
        let field1_nohtml = strip_html_preserving_media_filenames(&self.fields[0]);
        let checksum = field_checksum(field1_nohtml.as_ref());

        // always sort by first field
        let sort_field = field1_nohtml;

        if let Some(nid) = self.nid {
            // update note
            let mut stmt = trans.prepare_cached(include_str!("../sql/update_note.sql"))?;

            stmt.execute(params![
                self.guid,
                self.ntid,
                self.mtime,
                self.usn,
                join_tags(&self.tags),
                join_fields(&self.fields),
                sort_field,
                checksum,
                nid
            ])?;
        } else {
            // create note
            let mut stmt = trans.prepare_cached(include_str!("../sql/add_note.sql"))?;

            stmt.execute(params![
                1, // try 1 and if taken max+1
                self.guid,
                self.ntid,
                self.mtime,
                self.usn,
                join_tags(&self.tags),
                join_fields(&self.fields),
                sort_field,
                checksum,
            ])?;

            self.nid = Some(trans.last_insert_rowid());

            // write cards to db
            if let Some(ref mut cards) = self.cards {
                for card in cards {
                    if card.nid.is_none() {
                        card.nid = self.nid;
                    }
                    card.write_to_db(trans)?;
                }
            }
        }

        Ok(self.nid.unwrap())
    }

    pub fn get_id(&self) -> Option<i64> {
        self.nid.clone()
    }

    pub fn delete_with_cards(mut self, trans: &Transaction) -> Result<(), Error> {
        // do nothing if not written to db
        if let Some(nid) = self.nid {
            if self.cards.is_none() {
                self.load_cards(trans)?;
            }

            for card in self.cards.unwrap() {
                card.delete(trans)?;
            }

            add_grave(trans, nid, self.usn, GraveKind::Note)?;
            trans
                .prepare_cached("delete from notes where id = ?")?
                .execute([nid])?;
        }
        Ok(())
    }
}

fn base91_u64() -> String {
    anki_base91(rand::random())
}

fn anki_base91(n: u64) -> String {
    to_base_n(
        n,
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ\
0123456789!#$%&()*+,-./:;<=>?@[]^_`{|}~",
    )
}

fn to_base_n(mut n: u64, table: &[u8]) -> String {
    let mut buf = String::new();
    while n > 0 {
        let tablelen = table.len() as u64;
        let (q, r) = (n / tablelen, n % tablelen);
        buf.push(table[r as usize] as char);
        n = q;
    }
    buf.chars().rev().collect()
}

fn join_tags(tags: &[String]) -> String {
    if tags.is_empty() {
        "".into()
    } else {
        format!(" {} ", tags.join(" "))
    }
}

fn split_tags(tags: &str) -> impl Iterator<Item = &str> {
    tags.split(is_tag_separator).filter(|tag| !tag.is_empty())
}

fn is_tag_separator(c: char) -> bool {
    c == ' ' || c == '\u{3000}'
}

fn join_fields(fields: &[String]) -> String {
    fields.join("\x1f")
}

fn split_fields(fields: &str) -> Vec<String> {
    fields.split('\x1f').map(Into::into).collect()
}

fn field_checksum(text: &str) -> u32 {
    let mut hash = Sha1::new();
    hash.update(text);
    let digest = hash.finalize();
    u32::from_be_bytes(digest[..4].try_into().unwrap())
}
