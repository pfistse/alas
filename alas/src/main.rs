use clap::{Parser, Subcommand};
use std::process::Command;

use alas::{
    error::handle_error,
    init_dir,
    messages::{print_message, MessageType},
    sync_notes,
};

#[derive(Parser)]
#[command(name = "alas", about = "Sync LaTeX notes with Anki")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize alas for the current directory")]
    Init {
        #[arg(short, long, help = "Specify the Anki profile for synchronization")]
        profile: String,
        #[arg(short, long, help = "Specify the name of the Anki deck")]
        deck: Option<String>,
        #[arg(short, long, help = "Specify the technical name for Anki objects")]
        identifier: Option<String>,
        #[arg(short, long, help = "Add template .tex files", default_value_t = false)]
        files: bool,
    },
    #[command(about = "Sync all your LaTeX notes with Anki")]
    Sync {
        #[arg(short, long, help = "Specify the batch size", default_value = "9")]
        batch_size: usize,
    },
}

fn main() {
    check_required_programms();

    let cli = Cli::parse();
    if let Err(err) = match cli.command {
        Commands::Init {
            profile,
            deck,
            identifier,
            files,
        } => init_dir(profile, deck, identifier, files),
        Commands::Sync { batch_size } => sync_notes(batch_size),
    } {
        handle_error(err);
        std::process::exit(1);
    }
}

fn check_required_programms() {
    let required_programs = vec!["latex", "dvisvgm"];

    for program in required_programs {
        if !check_program_installed(program) {
            print_message(
                MessageType::Error,
                &format!("'{}' is not installed or not found in PATH.", program),
            );
            std::process::exit(1);
        }
    }
}

fn check_program_installed(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok()
}
