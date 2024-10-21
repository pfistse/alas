use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::error::Error;
use crate::note::Note;

pub enum NoteState {
    Unchanged,
    Changed,
    New,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    #[serde(skip)]
    pub config_path: PathBuf,
    #[serde(skip)]
    pub synced_notes: Vec<String>,
    pub anki_path: PathBuf,
    pub anki_profile: String,
    pub anki_deck_name: String,
    pub anki_identifier: String,
    pub anki_deck_id: Option<i64>,
    pub anki_notetype_id: Option<i64>,
    note_hashes: HashMap<String, String>,
    anki_notes: HashMap<String, i64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_path: PathBuf::from(".alas/config.json"),
            synced_notes: Vec::new(),
            anki_path: dirs::data_dir()
                .expect("failed to locate data directory")
                .join("Anki2"),
            // anki_path: PathBuf::from("/mnt/c/Users/sebas/AppData/Roaming/Anki2"),
            anki_profile: String::from("Test"),
            anki_deck_name: String::new(),
            anki_identifier: String::new(),
            anki_deck_id: None,
            anki_notetype_id: None,
            note_hashes: HashMap::new(),
            anki_notes: HashMap::new(),
        }
    }
}

impl Config {
    pub fn create(
        config_path: Option<PathBuf>,
        anki_deck_name: String,
        anki_identifier: String,
        anki_profile: String,
    ) -> Result<Self, Error> {
        let config_path = config_path.unwrap_or_else(|| PathBuf::from(".alas/config.json"));

        if Path::new(&config_path).exists() {
            return Err(Error::ConfigError(
                "Config file already exists.".to_string(),
            ));
        }

        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let config = Self {
            config_path,
            anki_deck_name,
            anki_identifier,
            anki_profile,
            ..Default::default()
        };

        Ok(config)
    }

    pub fn load(path: Option<&PathBuf>) -> Result<Self, Error> {
        let path = match path {
            Some(p) => p.clone(),
            None => PathBuf::from(".alas/config.json"),
        };

        let data = fs::read_to_string(&path)
            .map_err(|_| Error::ConfigError("Directory is not initialized.".to_string()))?;
        let mut config: Config = serde_json::from_str(&data)
            .map_err(|e| Error::ConfigError(format!("Config file is corrupted: {}", e)))?;

        config.config_path = path;
        Ok(config)
    }

    pub fn write_back(&self) -> Result<(), Error> {
        let data = serde_json::to_string_pretty(self)?;
        fs::write(&self.config_path, data)?;
        Ok(())
    }

    pub fn start_check_in(&mut self) {
        self.synced_notes = Vec::new();
    }

    pub fn check_in_note(&mut self, note: &Note) -> NoteState {
        self.synced_notes.push(note.id.clone());
        match self.note_hashes.get(&note.id) {
            Some(existing_hash) if existing_hash == &note.hash_text() => NoteState::Unchanged,
            Some(_) => NoteState::Changed,
            None => NoteState::New,
        }
    }

    pub fn update_note_state(&mut self, note: &Note) {
        self.note_hashes.insert(note.id.clone(), note.hash_text());
    }

    pub fn store_ankinote_id(&mut self, note: &Note, ankinote_id: i64) {
        self.anki_notes.insert(note.id.clone(), ankinote_id);
    }

    pub fn remove_note(&mut self, note_id: &str) {
        self.note_hashes.remove(note_id);
        self.anki_notes.remove(note_id);
    }

    pub fn get_ankinote_id(&self, note_id: &str) -> Option<i64> {
        self.anki_notes.get(note_id).copied()
    }

    pub fn get_unsynced_note_ids(&self) -> Vec<String> {
        self.note_hashes
            .keys()
            .filter(|id| !self.synced_notes.contains(id))
            .cloned()
            .collect()
    }
}
