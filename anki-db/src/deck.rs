// Code in this file is based on or derived from the Anki project.
// You can find the original code at https://github.com/ankitects/anki.

use prost::Message;
use rusqlite::{params, Transaction};

pub use anki_proto::decks::deck::{
    kind_container::Kind as KindProto, Common as CommonProto, KindContainer as KindContainerProto,
    Normal as NormalProto,
};

use crate::{
    deckconfig::DeckConfig,
    Error,
};

#[derive(Debug, Clone, PartialEq)]
pub struct AnkiDeck {
    pub did: Option<i64>,
    pub name: String,
    pub mtime: i64,
    pub usn: i64,
    pub common: CommonProto,
    pub kind: Option<KindProto>, // contains deck id, therefore option
    pub conf: DeckConfig,
}

impl AnkiDeck {
    pub fn new(name: &str) -> Self {
        Self {
            did: None,
            name: name.to_string(),
            mtime: 0,
            usn: -1,
            common: CommonProto {
                study_collapsed: true,
                browser_collapsed: true,
                ..Default::default()
            },
            kind: None,
            conf: DeckConfig {
                name: name.to_string(),
                ..Default::default()
            },
        }
    }

    pub fn load(did: i64, trans: &Transaction) -> Result<Option<Self>, Error> {
        let mut stmt = trans.prepare_cached(include_str!("../sql/get_deck.sql"))?;
        let mut rows = stmt.query(params![did])?;

        // deck with id found?
        let row = match rows.next()? {
            Some(row) => row,
            None => return Ok(None),
        };

        let common = CommonProto::decode(row.get_ref_unwrap(4).as_blob()?)?;

        let kind = KindContainerProto::decode(row.get_ref_unwrap(5).as_blob()?)?;

        let config_id = match &kind.kind {
            Some(KindProto::Normal(normal)) => normal.config_id,
            _ => panic!("wrong kind type"),
        };

        let conf =
            DeckConfig::load(config_id, trans)?.expect("deck references non-existing deckconfig");

        Ok(Some(AnkiDeck {
            did: Some(row.get(0)?),
            name: row.get(1)?,
            mtime: row.get(2)?,
            usn: row.get(3)?,
            common,
            kind: Some(kind.kind.unwrap()),
            conf,
        }))
    }

    pub fn write_to_db(&mut self, trans: &Transaction) -> Result<i64, Error> {
        // write deckconfig to db
        self.conf.write_to_db(trans)?;

        self.kind = Some(KindProto::Normal(NormalProto {
            config_id: self.conf.get_id().expect("deckconfig not written to db"),
            ..Default::default()
        }));

        // write deck to db
        let mut common = vec![];
        self.common.encode(&mut common)?;

        let kind_enum = KindContainerProto {
            kind: self.kind.clone(),
        };
        let mut kind = vec![];
        kind_enum.encode(&mut kind)?;

        let mut stmt = trans.prepare_cached(include_str!("../sql/add_deck.sql"))?;
        stmt.execute(params![
            self.did, self.name, self.mtime, self.usn, common, kind
        ])?;

        self.did = Some(trans.last_insert_rowid());

        Ok(self.did.unwrap())
    }

    pub fn get_id(&self) -> Option<i64> {
        self.did
    }
}
