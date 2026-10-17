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
    NoHome,
    DaysAndSince,
    BadAgents { given: String },
    UntilAlone,
    ProjectFolder { dir: PathBuf },
    NoProjectsDir { dir: PathBuf },
    NoHub { path: PathBuf },
    OllamaDown { url: String, why: String },
    ModelNotPulled { url: String, model: String },
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
                "lightsout: unknown flag '{given}'. runs takes --days N or --since/--until, --agent NAMES, --projects DIR and --json; andon takes --days N, --hub-db PATH and --json; escapes takes --days N or --since/--until, --agent NAMES, --projects DIR, --model, --ollama and --json"
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
                "lightsout: {} is not a command hub database this version can read ({why}). Pass --hub-db PATH to the hub's hub.db, or use 'runs', which needs only your transcripts",
                path.display()
            ),
            Error::DaysAndSince => write!(
                f,
                "lightsout: --days and --since both set the start of the window. Use one: --days 7, or --since 2026-10-10T00:00:00Z"
            ),
            Error::BadAgents { given } => write!(
                f,
                "lightsout: --agent takes agent names split by commas, with none empty; got '{given}'. Try --agent Keel,Harbor"
            ),
            Error::UntilAlone => write!(
                f,
                "lightsout: --until ends a window that --since starts. Add --since, e.g. --since 2026-10-17T02:00:00Z --until 2026-10-17T10:00:00Z"
            ),
            Error::ProjectFolder { dir } => write!(
                f,
                "lightsout: {} holds transcripts (*.jsonl) directly, but --projects wants the folder one level up, the one that holds a folder per project. Try --projects {}",
                dir.display(),
                dir.parent().map(|p| p.display().to_string()).unwrap_or_default()
            ),
            Error::NoHome => write!(
                f,
                "lightsout: HOME is not set, so there is no default folder for Claude Code transcripts. Pass --projects DIR, for example --projects ~/.claude/projects"
            ),
            Error::NoProjectsDir { dir } => write!(
                f,
                "lightsout: no Claude Code transcripts folder at {}. Claude Code writes one there after your first session; use it once and run again, or pass --projects DIR (or set CLAUDE_CONFIG_DIR if you moved it)",
                dir.display()
            ),
            Error::NoHub { path } => write!(
                f,
                "lightsout: andon reads the command hub's database, and there is none at {}. It is the one gauge that needs that hub, which only the machine that runs it has. Use 'lightsout runs' and 'lightsout escapes', which need only your transcripts, or pass --hub-db PATH if you run a hub",
                path.display()
            ),
            Error::OllamaDown { url, why } => write!(
                f,
                "lightsout: escapes labels your turns with a local model through Ollama, and nothing answered at {url} ({why}). Install Ollama from https://ollama.com, start it (ollama serve), then pull a model (ollama pull qwen3.5:9b-q8_0), or pass --ollama URL. Nothing leaves your machine"
            ),
            Error::ModelNotPulled { url, model } => write!(
                f,
                "lightsout: Ollama at {url} is running but does not have the model {model}. Run: ollama pull {model} (models are several GB), or pass --model with one you have (ollama list)"
            ),
        }
    }
}
