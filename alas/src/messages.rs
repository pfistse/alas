use colored::*;

pub enum MessageType {
    Info,
    Warning,
    Error,
}

pub fn print_message(message_type: MessageType, message: &str) {
    let prefix = match message_type {
        MessageType::Info => "Info".bold().white(),
        MessageType::Warning => "Warning".bold().yellow(),
        MessageType::Error => "Error".bold().red(),
    };

    println!("{}: {}", prefix, message);
}
