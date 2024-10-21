use colored::{Color, Colorize};
use crossterm::{cursor, execute, terminal, ExecutableCommand};
use linked_hash_map::LinkedHashMap;
use std::{
    fmt,
    io::{stdout, Stdout, Write},
};

use crate::note::Note;

#[derive(Clone)] // TODO no clone necessary
pub enum AnkiJob {
    Modify(Note, ModifyAction, JobState), // TODO make Note read only -> Rc
    Delete(String, JobState),
}

impl AnkiJob {
    pub fn change_state(&mut self, new_state: JobState) {
        match self {
            AnkiJob::Modify(_, _, state) => *state = new_state,
            AnkiJob::Delete(_, state) => *state = new_state,
        }
    }
}

#[derive(Clone)]
pub enum ModifyAction {
    Add,
    Update,
}

#[derive(Clone)]
pub enum JobState {
    Detected,
    Processing,
    Success,
    Failed(String),
}

impl fmt::Display for AnkiJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let formatted = match self {
            AnkiJob::Modify(note, ModifyAction::Add, state) => {
                let (msg, col) = match state {
                    JobState::Detected => (
                        "detected",
                        Color::TrueColor {
                            r: 162,
                            g: 162,
                            b: 162,
                        },
                    ),
                    JobState::Processing => ("processing", Color::BrightGreen),
                    JobState::Success => ("added", Color::BrightGreen),
                    JobState::Failed(msg) => {
                        (msg.as_str(), Color::TrueColor { r: 160, g: 0, b: 0 })
                    }
                };
                format!("+ {} {}", &note.id, msg).color(col)
            }
            AnkiJob::Modify(note, ModifyAction::Update, state) => {
                let (msg, col) = match state {
                    JobState::Detected => (
                        "detected",
                        Color::TrueColor {
                            r: 162,
                            g: 162,
                            b: 162,
                        },
                    ),
                    JobState::Processing => ("processing", Color::BrightBlue),
                    JobState::Success => ("updated", Color::BrightBlue),
                    JobState::Failed(msg) => {
                        (msg.as_str(), Color::TrueColor { r: 160, g: 0, b: 0 })
                    }
                };
                format!("~ {} {}", &note.id, msg).color(col)
            }
            AnkiJob::Delete(note_id, state) => {
                let (msg, col) = match state {
                    JobState::Detected => (
                        "detected",
                        Color::TrueColor {
                            r: 162,
                            g: 162,
                            b: 162,
                        },
                    ),
                    JobState::Processing => panic!("instant operations are in no processing state"),
                    JobState::Success => ("deleted", Color::BrightRed),
                    JobState::Failed(msg) => {
                        (msg.as_str(), Color::TrueColor { r: 255, g: 0, b: 0 })
                    }
                };
                format!("- {} {}", &note_id, msg).color(col)
            }
        };

        write!(f, "{}", formatted)
    }
}

pub struct JobMonitor {
    cells: LinkedHashMap<String, AnkiJob>,
    num_columns: usize,
    stdout: Stdout, // TODO interior mutability for stdout?
}

impl JobMonitor {
    pub fn new(num_columns: usize) -> Self {
        let mut monitor = JobMonitor {
            cells: LinkedHashMap::new(),
            num_columns,
            stdout: stdout(),
        };

        execute!(monitor.stdout, terminal::Clear(terminal::ClearType::All)).unwrap();
        execute!(monitor.stdout, cursor::Hide).unwrap();

        monitor
    }

    pub fn update(&mut self, jobs: &Vec<AnkiJob>) {
        for job in jobs {
            match job {
                AnkiJob::Modify(note, _, _) => {
                    self.cells
                        .entry(note.id.clone())
                        .and_modify(|existing_job| *existing_job = job.clone())
                        .or_insert_with(|| job.clone());
                }
                AnkiJob::Delete(note_id, _) => {
                    self.cells
                        .entry(note_id.clone())
                        .and_modify(|existing_job| *existing_job = job.clone())
                        .or_insert_with(|| job.clone());
                }
            }
        }

        self.display();
    }

    fn display(&mut self) {
        self.stdout.execute(cursor::MoveTo(0, 0)).unwrap();

        for (i, job) in self.cells.values().enumerate() {
            let row = i / self.num_columns;
            let col = i % self.num_columns;

            let col_width = 30;
            self.stdout
                .execute(cursor::MoveTo((col * col_width) as u16, row as u16))
                .unwrap();

            write!(
                self.stdout,
                "{:<width$}",
                job.to_string(),
                width = col_width
            )
            .unwrap(); // TODO avoid to_string?
        }
        self.stdout.flush().unwrap();
    }

    pub fn close(mut self) {
        self.stdout.execute(cursor::Show).unwrap();
        println!();
    }
}
