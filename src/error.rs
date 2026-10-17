//! Every error lightsout can show, with its words written once here. Each names what failed, the rule it broke and what to do next.

use std::fmt;
use std::path::PathBuf;

pub enum Error {
    UnknownCommand { given: String },
    MissingValue { flag: &'static str },
    BadDays { given: String },
    UnknownFlag { given: String },
    NoTranscripts { dir: PathBuf },
    ReadDir { dir: PathBuf, why: String },
    HubDb { path: PathBuf, why: String },
    Ollama { url: String, model: String, why: String },
    BadInstant { flag: &'static str, given: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownCommand { given } => write!(
                f,
                "lightsout: unknown command '{given}'. The commands are 'runs', 'andon' and 'escapes'. Try: lightsout runs --days 7"
            ),
            Error::MissingValue { flag } => {
                write!(f, "lightsout: {flag} needs a value after it, for example {flag} 7")
            }
            Error::BadDays { given } => write!(
                f,
                "lightsout: --days takes a whole number of days above 0; got '{given}'. Try --days 7"
            ),
            Error::UnknownFlag { given } => write!(
                f,
                "lightsout: unknown flag '{given}'. runs takes --days N, --projects DIR and --json; andon takes --days N, --hub-db PATH and --json; escapes takes --days N or --since/--until, --projects DIR, --model, --ollama and --json"
            ),
            Error::NoTranscripts { dir } => write!(
                f,
                "lightsout: no Claude Code transcripts (*.jsonl) changed in the window under {}. Widen it with --days, or point --projects at the folder that holds them",
                dir.display()
            ),
            Error::ReadDir { dir, why } => write!(
                f,
                "lightsout: can't read {}: {why}. Check the path, or pass --projects DIR",
                dir.display()
            ),
            Error::Ollama { url, model, why } => write!(
                f,
                "lightsout: the local model {model} at {url} gave no label: {why}. Start Ollama (ollama serve), pull the model (ollama pull {model}), or pass --model and --ollama"
            ),
            Error::BadInstant { flag, given } => write!(
                f,
                "lightsout: {flag} takes a UTC time like 2026-10-17T07:00:00Z; got '{given}'"
            ),
            Error::HubDb { path, why } => write!(
                f,
                "lightsout: can't read the hub database at {}: {why}. Pass --hub-db PATH, or run 'runs', which needs no hub",
                path.display()
            ),
        }
    }
}
