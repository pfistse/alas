// Code in this file is based on or derived from the Anki project.
// You can find the original code at https://github.com/ankitects/anki.

use prost::Message;
use rusqlite::{params, OptionalExtension, Transaction};

pub use anki_proto::notetypes::notetype::{
    field::Config as FieldConfigProto, template::Config as TemplateConfigProto,
    Config as ConfigProto,
};

use crate::Error;

#[derive(Debug, PartialEq, Clone)]
pub struct Notetype {
    ntid: Option<i64>,
    name: String,
    mtime: i64,
    usn: i32,
    fields: Vec<Field>,
    templates: Vec<Template>,
    config: ConfigProto,
}

#[derive(Debug, PartialEq, Clone)]
struct Field {
    name: String,
    config: FieldConfigProto,
}

#[derive(Debug, PartialEq, Clone)]
struct Template {
    mtime: i64,
    usn: i32,
    name: String,
    config: TemplateConfigProto,
}

impl Notetype {
    pub fn new(name: &str) -> Self {
        Self {
            ntid: None,
            name: name.to_string(),
            mtime: 0,
            usn: -1,
            fields: vec![],
            templates: vec![],
            config: Notetype::default_config(),
        }
    }

    pub fn load(ntid: i64, trans: &Transaction) -> Result<Option<Self>, Error> {
        let fields = trans
            .prepare_cached(include_str!("../sql/get_fields.sql"))?
            .query_and_then(params![ntid], |row| -> Result<Field, Error> {
                let config = FieldConfigProto::decode(row.get_ref_unwrap(2).as_blob()?)?;
                Ok(Field {
                    name: row.get(1)?,
                    config,
                })
            })?
            .collect::<Result<Vec<Field>, Error>>()?;

        let templates = trans
            .prepare_cached(include_str!("../sql/get_template.sql"))?
            .query_and_then(params![ntid], |row| -> Result<Template, Error> {
                let config = TemplateConfigProto::decode(row.get_ref_unwrap(4).as_blob()?)?;
                Ok(Template {
                    name: row.get(1)?,
                    mtime: row.get(2)?,
                    usn: row.get(3)?,
                    config,
                })
            })?
            .collect::<Result<Vec<Template>, Error>>()?;

        trans
            .prepare_cached(include_str!("../sql/get_notetype.sql"))?
            .query_row(params![ntid], |row| {
                let config = ConfigProto::decode(row.get_ref_unwrap(4).as_blob()?)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                Ok(Notetype {
                    ntid: Some(row.get(0)?),
                    name: row.get(1)?,
                    mtime: row.get(2)?,
                    usn: row.get(3)?,
                    config,
                    fields,
                    templates,
                })
            })
            .optional()
            .map_err(Error::from)
    }

    pub fn with_field(mut self, name: &str) -> Self {
        self.fields.push(Field {
            name: name.to_string(),
            config: FieldConfigProto {
                id: Some(rand::random()),
                sticky: false,
                rtl: false,
                plain_text: false,
                font_name: "Arial".into(),
                font_size: 20,
                description: "".into(),
                collapsed: false,
                exclude_from_search: false,
                tag: None,
                prevent_deletion: false,
                other: vec![],
            },
        });
        self
    }

    pub fn with_template(mut self, name: &str, qfmt: &str, afmt: &str, did: i64) -> Self {
        self.templates.push(Template {
            name: name.to_string(),
            usn: -1,
            mtime: 0,
            config: TemplateConfigProto {
                id: Some(rand::random()),
                q_format: qfmt.to_string(),
                a_format: afmt.to_string(),
                q_format_browser: "".to_string(),
                a_format_browser: "".to_string(),
                target_deck_id: did,
                browser_font_name: "".to_string(),
                browser_font_size: 0,
                other: vec![],
            },
        });
        self
    }

    pub fn num_templates(&self) -> usize {
        self.templates.len()
    }

    fn default_config() -> ConfigProto {
        ConfigProto {
            css: include_str!("../templates/notetype_css.txt").to_string(),
            latex_pre: include_str!("../templates/notetype_latex_pre.txt").to_string(),
            latex_post: include_str!("../templates/notetype_latex_post.txt").to_string(),
            ..Default::default()
        }
    }

    pub fn write_to_db(&mut self, trans: &Transaction) -> Result<i64, Error> {

        let mut config_bytes = vec![];
        self.config.encode(&mut config_bytes)?;

        if let Some(ntid) = self.ntid {
            // update notetype
            trans
                .prepare_cached(include_str!("../sql/update_notetype.sql"))?
                .execute(params![self.name, self.mtime, self.usn, config_bytes, ntid])?;
        } else {
            // add notetype
            trans
                .prepare_cached(include_str!("../sql/add_notetype.sql"))?
                .execute(params![1, self.name, self.mtime, self.usn, config_bytes])?;

            self.ntid = Some(trans.last_insert_rowid());
        }

        // update fields
        trans
            .prepare_cached("delete from fields where ntid=?")?
            .execute([self.ntid])?;

        let mut stmt = trans.prepare_cached(include_str!("../sql/add_fields.sql"))?;

        for (ord, field) in self.fields.iter().enumerate() {
            let mut config_bytes = vec![];
            field.config.encode(&mut config_bytes)?;

            stmt.execute(params![
                self.ntid.expect("notetype not written to db"),
                ord as u32,
                field.name,
                config_bytes,
            ])?;
        }

        // update templates
        trans
            .prepare_cached("delete from templates where ntid=?")?
            .execute([self.ntid])?;

        let mut stmt = trans.prepare_cached(include_str!("../sql/add_templates.sql"))?;

        for (ord, template) in self.templates.iter().enumerate() {
            let mut config_bytes = vec![];
            template.config.encode(&mut config_bytes)?;

            stmt.execute(params![
                self.ntid.expect("notetype not written to db"),
                ord as u32,
                template.name,
                template.mtime,
                template.usn,
                config_bytes,
            ])?;
        }

        Ok(self.ntid.unwrap())
    }

    pub fn get_id(&self) -> Option<i64> {
        self.ntid
    }
}
