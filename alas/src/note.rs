use lazy_static::lazy_static;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Read, Write},
    path::PathBuf,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Note {
    pub id: String,
    pub note_type: Option<String>,
    pub fields: Vec<String>,
}

impl Note {
    pub fn hash_text(&self) -> String {
        let mut hasher = Sha256::new();
        let concatenated_fields = self.fields.join("|");
        hasher.update(concatenated_fields.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

lazy_static! {
    static ref NOTE_RE: Regex =
        Regex::new(r"(?sm)^[^%\n]*?\\begin\{note\}(?:\[(.*?)\])?(.*?)\\end\{note\}").unwrap();
    static ref FIELD_RE: Regex =
        Regex::new(r"(?sm)^[^%\n]*?\\begin\{field\}\s*(.*?)\s*\\end\{field\}").unwrap();
    static ref ID_RE: Regex = Regex::new(r"%\s*ID:\s*([^\r\n]*)").unwrap();
}

pub fn parse_tex_file(path: &PathBuf) -> Vec<Note> {
    let file = File::open(&path).expect("Could not open file");
    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader
        .read_to_string(&mut content)
        .expect("Could not read file");

    let mut notes = Vec::new();

    let mut last_pos = 0;

    // iterate through each captured note
    for cap in NOTE_RE.captures_iter(&content) {
        let start = cap.get(0).unwrap().start();
        let end = cap.get(0).unwrap().end();

        // extract the ID from the comment before the note
        let before_note = &content[last_pos..start];
        let id = ID_RE
            .captures(before_note)
            .expect("id should have been inserted")
            .get(1)
            .unwrap()
            .as_str()
            .to_string();

        // capture note type and note body
        let note_type = cap.get(1).map(|note_type| note_type.as_str().to_string());
        let note_body = cap.get(2).map_or("", |m| m.as_str());

        // extract fields from the note body
        let mut fields = Vec::new();
        for field_cap in FIELD_RE.captures_iter(note_body) {
            let field_content = field_cap.get(1).map_or("", |m| m.as_str()).to_string();
            fields.push(field_content);
        }

        let note = Note {
            id,
            note_type,
            fields,
        };

        notes.push(note);
        last_pos = end;
    }

    notes
}

pub fn insert_id_if_missing(path: &PathBuf) -> Result<(), std::io::Error> {
    let file = File::open(&path)?;
    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader.read_to_string(&mut content)?;

    let mut output = String::new();
    let mut id_injected = false;

    let mut last_pos = 0;
    for cap in NOTE_RE.captures_iter(&content) {
        let start = cap.get(0).unwrap().start();
        let end = cap.get(0).unwrap().end();

        // check for ID before the note environment
        let before_note = &content[last_pos..start];
        let id_exists = ID_RE.is_match(before_note);

        // write content before the note starts
        output.push_str(&content[last_pos..start]);

        if !id_exists {
            // if no ID comment exists, generate a new one and inject it
            let new_id = Uuid::new_v4().to_string()[..8].to_string();
            output.push_str(&format!("% ID: {}\n", new_id));
            id_injected = true;
        }

        // write the note environment
        output.push_str(&content[start..end]);
        last_pos = end;
    }

    // append any remaining content after the last match
    output.push_str(&content[last_pos..]);

    if id_injected {
        let mut out_file = OpenOptions::new().write(true).truncate(true).open(&path)?;
        write!(out_file, "{}", output)?;
    }

    Ok(())
}
