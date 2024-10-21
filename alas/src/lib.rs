#![feature(mpmc_channel)]

use convert_case::{Case, Casing};
use include_dir::{include_dir, Dir};
use rusqlite::{self, Connection, Transaction};
use std::{env, fs, path::PathBuf, process::Command};
use unicase::UniCase;

use anki_db::{self, AnkiDeck, Note as AnkiNote, Notetype as AnkiNotetype};

use config::{Config, NoteState};
use jobs::{AnkiJob, JobMonitor, JobState, ModifyAction};
use note::Note;

pub use error::Error;

mod config;
mod jobs;
mod note;

pub mod error;
pub mod messages;

pub fn init_dir(
    anki_profile: String,
    deck_name: Option<String>,
    identifier: Option<String>,
    files: bool,
) -> Result<(), Error> {
    let deck_name = deck_name.unwrap_or_else(|| {
        get_current_dir()
            .unwrap_or_else(|| format!("Deck {}", rand::random::<u64>()))
            .to_case(Case::Title)
    });

    let identifier = identifier.unwrap_or_else(|| deck_name.to_case(Case::Kebab)); // TODO non injective mapping from deck name to identifier

    let mut config = Config::create(None, deck_name, identifier, anki_profile)?;

    if files {
        init_project_dir()?;
    }

    let anki_db_path = config
        .anki_path
        .join(&config.anki_profile)
        .join("collection.anki2");

    let mut conn = rusqlite::Connection::open(anki_db_path)
        .map_err(|_| Error::AlasError("Failed to open an anki database connection.".to_string()))?;

    conn.create_collation("unicase", |lhs: &str, rhs: &str| {
        UniCase::new(lhs).cmp(&UniCase::new(rhs))
    })
    .expect("can register unicase");

    // check db scheme compatibility
    if !anki_db::check_db_compatibility(&mut conn)? {
        return Err(Error::AlasError(
            "Your Anki version is not compatible with alas. Try upgrading Anki.".to_string(),
        ));
    }

    let trans = new_transaction(&mut conn)?;

    let mut deck = AnkiDeck::new(&config.anki_deck_name);
    config.anki_deck_id = Some(deck.write_to_db(&trans)?);

    let mut notetype = AnkiNotetype::new(&format!("{}-notetype", &config.anki_identifier))
        .with_field("front")
        .with_field("back")
        .with_template(
            &format!("{}-template", &config.anki_identifier),
            include_str!("../templates/anki/minimal_front.txt"),
            include_str!("../templates/anki/minimal_back.txt"),
            config.anki_deck_id.expect("deck is written to db"),
        );

    config.anki_notetype_id = Some(notetype.write_to_db(&trans)?);

    trans.commit()?;
    config.write_back()?;
    Ok(())
}

pub fn get_current_dir() -> Option<String> {
    Some(env::current_dir().ok()?.file_name()?.to_str()?.to_string())
}

pub fn sync_notes(batch_size: usize) -> Result<(), Error> {
    let mut config = Config::load(None)?;

    let tex_files = find_tex_files_in_current_dir()?;

    let anki_db_path = config
        .anki_path
        .join(&config.anki_profile)
        .join("collection.anki2");
    let anki_media_dir = config
        .anki_path
        .join(&config.anki_profile)
        .join("collection.media");

    // TODO function for connection and transaction
    let mut conn = rusqlite::Connection::open(anki_db_path)
        .map_err(|_| Error::AlasError("Failed to open an anki database connection.".to_string()))?;

    conn.create_collation("unicase", |lhs: &str, rhs: &str| {
        UniCase::new(lhs).cmp(&UniCase::new(rhs))
    })
    .expect("fail register unicase");

    let trans = new_transaction(&mut conn)?;

    let deck = AnkiDeck::load(config.anki_deck_id.expect("deck is written to db"), &trans)?
        .ok_or_else(|| Error::AlasError("Anki deck could not be found.".to_string()))?;

    // TODO do not load notetype from db, just use id from config
    let notetype_id = config.anki_notetype_id.expect("notetype is written to db");
    let notetype = AnkiNotetype::load(notetype_id, &trans)?
        .ok_or_else(|| Error::AlasError("Anki note could not be found.".to_string()))?;

    let notes = tex_files
        .iter()
        .map(|file| {
            note::insert_id_if_missing(file)?;
            Ok(note::parse_tex_file(file))
        })
        .collect::<Result<Vec<_>, Error>>()?
        .into_iter()
        .flatten();

    trans.commit()?;

    let mut monitor = JobMonitor::new(3);

    config.start_check_in(); // TODO change module name: config -> ??? (logging, tracking, ...)

    // TODO own type for modify jobs?
    let mut modify_jobs = notes
        .filter_map(|n| match config.check_in_note(&n) {
            NoteState::New => Some(AnkiJob::Modify(n, ModifyAction::Add, JobState::Detected)),
            NoteState::Changed => {
                Some(AnkiJob::Modify(n, ModifyAction::Update, JobState::Detected))
            }
            NoteState::Unchanged => None,
        })
        .collect();

    let mut delete_jobs = config
        .get_unsynced_note_ids()
        .into_iter()
        .map(|nid| AnkiJob::Delete(nid, JobState::Detected))
        .collect();

    monitor.update(&modify_jobs);
    monitor.update(&delete_jobs);

    for job_chunk in modify_jobs.chunks_mut(batch_size) {
        let mut notes = Vec::new();

        for job in job_chunk.iter_mut() {
            job.change_state(JobState::Processing);
        }

        for job in job_chunk.iter() {
            // TODO: all jobs are modify jobs
            if let AnkiJob::Modify(note, _, _) = job {
                notes.push(note);
            }
        }

        monitor.update(&job_chunk.to_vec());

        // render batch; if one job fails all batch jobs fail
        let result = generate_tmp_svg_files_for_batch(&notes).and_then(|_| {
            move_tmp_svg_files_to_anki_media(&notes, &anki_media_dir, &config.anki_identifier)
        });
        match result {
            Err(Error::JobError(msg)) => {
                for job in job_chunk.iter_mut() {
                    job.change_state(JobState::Failed(msg.clone()));
                }
                monitor.update(&job_chunk.to_vec());
                continue;
            }
            r => r?,
        }

        // write notes to anki db
        for job in job_chunk.iter_mut() {
            let result = match job {
                AnkiJob::Modify(note, ModifyAction::Add, JobState::Processing) => {
                    add_note_to_anki(note, &notetype, &deck, &mut conn, &mut config)
                }
                AnkiJob::Modify(note, ModifyAction::Update, JobState::Processing) => {
                    update_note_in_anki(note, &notetype, &deck, &mut conn, &mut config)
                }
                _ => panic!("only modify jobs in list"),
            };

            match result {
                Ok(_) => {
                    job.change_state(JobState::Success);
                }
                Err(Error::JobError(msg)) => {
                    job.change_state(JobState::Failed(msg));
                }
                Err(err) => Err(err)?,
            }
        }

        monitor.update(&job_chunk.to_vec()); // TODO update after each job?
    }

    // process delete jobs
    for job in delete_jobs.iter_mut() {
        job.change_state(JobState::Processing);

        let result = match job {
            AnkiJob::Delete(note_id, JobState::Processing) => {
                delete_note_in_anki(note_id, &mut conn, &mut config)
            }
            _ => panic!("only delete jobs in list"),
        };

        match result {
            Ok(_) => {
                job.change_state(JobState::Success);
            }
            Err(Error::JobError(msg)) => {
                job.change_state(JobState::Failed(msg));
            }
            Err(err) => Err(err)?,
        }
    }

    monitor.update(&delete_jobs);
    monitor.close();

    clear_tmp_files()?;
    Ok(())
}

fn add_note_to_anki(
    note: &Note,
    notetype: &AnkiNotetype,
    deck: &AnkiDeck,
    conn: &mut Connection,
    config: &mut Config,
) -> Result<(), Error> {
    let trans = new_transaction(conn)?;

    let mut ankinote = AnkiNote::new(notetype.get_id().expect("notetype not written do db"));

    for i in 0..note.fields.len() {
        let entry = format!(
            "<img class=\"{}\" src=\"alas-{}-{}-{}.svg\">",
            note.note_type.as_deref().unwrap_or("default"),
            &config.anki_identifier,
            note.id,
            i
        );
        ankinote = ankinote.with_field_entry(&entry);
    } // testing

    let ankinote_id = ankinote
        .generate_cards(&notetype, &deck)
        .write_to_db(&trans)
        .map_err(|_| Error::JobError("db error".to_string()))?;

    config.store_ankinote_id(&note, ankinote_id);

    trans.commit()?;
    config.update_note_state(note);
    config.write_back()?;
    Ok(())
}

fn update_note_in_anki(
    note: &Note,
    notetype: &AnkiNotetype,
    deck: &AnkiDeck,
    conn: &mut Connection,
    config: &mut Config,
) -> Result<(), Error> {
    let trans = new_transaction(conn)?;

    let ankinote_id = config
        .get_ankinote_id(&note.id)
        .expect("inconsistent config");

    let mut ankinote = AnkiNote::load_without_cards(&trans, ankinote_id)
        .map_err(|_| Error::JobError("db error".to_string()))?
        .ok_or_else(|| Error::JobError("not found".to_string()))?
        .with_fields(vec![]);

    for i in 0..note.fields.len() {
        let entry = format!(
            "<img class=\"{}\" src=\"alas-{}-{}-{}.svg\">",
            note.note_type.as_deref().unwrap_or("default"),
            &config.anki_identifier,
            note.id,
            i
        );
        ankinote = ankinote.with_field_entry(&entry);
    }

    ankinote
        .generate_cards(&notetype, &deck)
        .write_to_db(&trans)
        .map_err(|_| Error::JobError("db error".to_string()))?;

    trans.commit()?;
    config.update_note_state(note);
    config.write_back()?;
    Ok(())
}

fn delete_note_in_anki(
    note_id: &str,
    conn: &mut Connection,
    config: &mut Config,
) -> Result<(), Error> {
    let trans = new_transaction(conn)?;

    let ankinote_id = config.get_ankinote_id(&note_id).expect("consistent config");
    let ankinote = AnkiNote::load_without_cards(&trans, ankinote_id)
        .map_err(|_| Error::JobError("db error".to_string()))?
        .ok_or_else(|| Error::JobError("not found".to_string()))?;

    ankinote
        .delete_with_cards(&trans)
        .map_err(|_| Error::JobError("db error".to_string()))?;

    trans.commit()?;
    config.remove_note(&note_id);
    config.write_back()?;
    Ok(())
}

fn generate_tmp_svg_files_for_batch(batch: &Vec<&Note>) -> Result<(), Error> {
    // TODO can one field exceed one page?
    let compiled = batch
        .iter()
        .flat_map(|note| &note.fields)
        .map(|field| field.replace("\\newpage", ""))
        .fold(String::from(""), |acc, f| format!("{acc} \n\\newpage {f}")); // TODO is format! efficient?

    let latex = include_str!("../templates/latex/skeleton.tex").replace("{{content}}", &compiled);

    fs::write("tmp.tex", latex)?;

    // generate dvi file
    Command::new("latex")
        .args(&["-interaction=nonstopmode", "tmp.tex"])
        .output()?;

    // generate one svg file for each page
    Command::new("dvisvgm")
        .args(&[
            "--no-fonts",
            "-Z",
            "2",
            "tmp.dvi",
            "--page=1-",
            "-o",
            "tmp-%3p.svg",
        ])
        .output()?;

    // assume rendering was successfull if there are 2 * #notes files
    // TODO can i identify the page which failed? is there a option to skip this page number?
    let last_file = format!("tmp-{:03}.svg", batch.len() * 2);
    if !PathBuf::from(&last_file).exists() {
        return Err(Error::JobError("failed rendering".to_string()));
    }

    Ok(())
}

fn move_tmp_svg_files_to_anki_media(
    notes: &Vec<&Note>,
    anki_media_dir: &PathBuf,
    identifier: &str,
) -> Result<(), Error> {
    let mut i = 1;
    for note in notes {
        for j in 0..note.fields.len() {
            let src = format!("tmp-{:03}.svg", i); // TODO batch larger than 99?
            let dest_file = format!("alas-{}-{}-{}.svg", identifier, note.id, j);
            let dest = anki_media_dir.join(dest_file);
            fs::copy(&src, &dest)?;
            fs::remove_file(&src)?;
            i += 1;
        }
    }
    Ok(())
}

fn clear_tmp_files() -> Result<(), std::io::Error> {
    for entry in fs::read_dir(".")? {
        let entry = entry?;
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .map_or(false, |name| {
                name.starts_with("tmp-") && name.ends_with(".svg")
            })
        {
            fs::remove_file(path)?;
        }
    }
    fs::remove_file("tmp.aux").ok();
    fs::remove_file("tmp.log").ok();
    fs::remove_file("tmp.tex").ok();
    fs::remove_file("tmp.dvi").ok();
    Ok(())
}

fn find_tex_files_in_current_dir() -> Result<Vec<PathBuf>, Error> {
    let mut tex_files = Vec::new();
    for entry in fs::read_dir(".")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("tex") {
            tex_files.push(path);
        }
    }
    Ok(tex_files)
}

fn new_transaction(conn: &mut Connection) -> Result<Transaction, Error> {
    conn.transaction()
        .map_err(|_| Error::AlasError("Failed to create an anki database transaction.".to_string()))
}

static TEMPLATE_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/templates/project_dir");

fn init_project_dir() -> Result<(), Error> {
    let current_dir = env::current_dir().expect("failed to get current directory");
    TEMPLATE_DIR
        .extract(&current_dir)
        .map_err(|_| Error::AlasError("Failed to initialize project directory.".to_string()))
}
