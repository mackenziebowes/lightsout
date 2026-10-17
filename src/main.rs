//! lightsout: how long coding agents run per human touch. `runs` reads Claude Code transcripts (any machine); `andon` reads this machine's command hub. Promise: read-only on everything it reads, and every gauge it cannot measure says so.

mod andon;
mod error;
mod escapes;
mod report;
mod scope;
mod session;
mod transcript;

use error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use scope::{Agents, Scope, Window};

enum Command {
    Runs { scope: Scope, projects: PathBuf, json: bool },
    Andon { days: u32, hub_db: PathBuf, json: bool },
    Escapes { scope: Scope, projects: PathBuf, model: String, ollama: String, json: bool },
    Help,
}

const USAGE: &str = "lightsout: how long coding agents run per human touch

  lightsout runs  [--days 7 | --since 2026-10-17T02:00:00Z [--until ...]] [--agent Keel,Harbor] [--projects ~/.claude/projects] [--json]
      agent minutes after each human turn, from Claude Code transcripts
  lightsout andon [--days 14] [--hub-db ~/.local/share/command-hub/hub.db] [--json]
      the command hub's asks, answer times, withdrawals and findings
  lightsout escapes [--days 1 | --since 2026-10-17T07:00:00Z [--until ...]] [--agent Keel,Harbor] [--model qwen3.5:9b-q8_0] [--ollama http://127.0.0.1:11434] [--json]
      share of human turns that say an agent's work was wrong, labelled by a local model (a floor)";

fn default_projects() -> Result<PathBuf, Error> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir).join("projects"));
    }
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).ok_or(Error::NoHome)?;
    Ok(PathBuf::from(home).join(".claude/projects"))
}

fn default_hub_db() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default().join(".local/share/command-hub/hub.db")
}

fn parse_args(args: &[String]) -> Result<Command, Error> {
    let Some(cmd) = args.first() else { return Ok(Command::Help) };
    if matches!(cmd.as_str(), "help" | "--help" | "-h") {
        return Ok(Command::Help);
    }
    if !matches!(cmd.as_str(), "runs" | "andon" | "escapes") {
        return Err(Error::UnknownCommand { given: cmd.to_string() });
    }
    let allowed: &[&str] = match cmd.as_str() {
        "runs" => &["--days", "--since", "--until", "--agent", "--projects", "--json"],
        "andon" => &["--days", "--hub-db", "--json"],
        _ => &["--days", "--since", "--until", "--agent", "--projects", "--model", "--ollama", "--json"],
    };
    let mut values: Vec<(&'static str, String)> = Vec::new();
    let mut json = false;
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        let Some(flag) = allowed.iter().find(|f| **f == a.as_str()).copied() else {
            return Err(Error::UnknownFlag { given: a.to_string() });
        };
        if flag == "--json" {
            json = true;
            continue;
        }
        let v = it.next().ok_or(Error::MissingValue { flag })?;
        values.push((flag, v.clone()));
    }
    let get = |flag: &str| values.iter().find(|(f, _)| *f == flag).map(|(_, v)| v.clone());
    let days = |default: u32| -> Result<u32, Error> {
        match get("--days") {
            None => Ok(default),
            Some(v) => v.parse::<u32>().ok().filter(|d| *d > 0).ok_or(Error::BadDays { given: v }),
        }
    };
    let instant = |flag: &'static str| -> Result<Option<transcript::Instant>, Error> {
        match get(flag) {
            None => Ok(None),
            Some(v) => transcript::Instant::parse(&v).map(Some).ok_or(Error::BadInstant { flag, given: v }),
        }
    };
    let projects = || match get("--projects") {
        Some(p) => Ok(PathBuf::from(p)),
        None => default_projects(),
    };
    if get("--days").is_some() && get("--since").is_some() {
        return Err(Error::DaysAndSince);
    }
    if get("--until").is_some() && get("--since").is_none() {
        return Err(Error::UntilAlone);
    }
    let scope = |default_days: u32| -> Result<Scope, Error> {
        let window = match instant("--since")? {
            Some(since) => Window::Span { since, until: instant("--until")? },
            None => Window::Days(days(default_days)?),
        };
        let agents = match get("--agent") {
            None => Agents::Any,
            Some(list) => Agents::parse(&list).ok_or(Error::BadAgents { given: list })?,
        };
        Ok(Scope { window, agents })
    };
    Ok(match cmd.as_str() {
        "runs" => Command::Runs { scope: scope(7)?, projects: projects()?, json },
        "andon" => Command::Andon {
            days: days(14)?,
            hub_db: get("--hub-db").map(PathBuf::from).unwrap_or_else(default_hub_db),
            json,
        },
        _ => Command::Escapes {
            scope: scope(1)?,
            projects: projects()?,
            model: get("--model").unwrap_or_else(|| "qwen3.5:9b-q8_0".into()),
            ollama: get("--ollama").unwrap_or_else(|| "http://127.0.0.1:11434".into()),
            json,
        },
    })
}

/// One transcript file: its turns, and the names Claude Code gave the session.
struct Session {
    events: Vec<transcript::Event>,
    names: Vec<String>,
}

fn read_session_file(f: &Path) -> Session {
    let text = std::fs::read_to_string(f).unwrap_or_default();
    Session {
        events: text.lines().filter_map(|l| transcript::parse_line(l).ok()).collect(),
        names: text.lines().filter_map(transcript::agent_name).collect(),
    }
}

/// Splits sessions into those the scope's agents admit and a count of those left out that had a turn in the window.
fn in_agents(files: &[PathBuf], scope: &Scope, now_ms: i64) -> (Vec<Session>, usize) {
    let (since, until) = scope.window.bounds(now_ms);
    let in_window = |s: &Session| {
        s.events.iter().any(|e| {
            let at = match e {
                transcript::Event::Input { at, .. } | transcript::Event::ToolResult { at } | transcript::Event::AgentTurn { at, .. } => *at,
            };
            at >= since && until.is_none_or(|u| at < u)
        })
    };
    let mut kept = Vec::new();
    let mut left_out = 0;
    for f in files {
        let s = read_session_file(f);
        if scope.agents.admits(&s.names) {
            kept.push(s);
        } else if in_window(&s) {
            left_out += 1;
        }
    }
    (kept, left_out)
}

fn transcripts(dir: &Path, since: SystemTime) -> Result<Vec<PathBuf>, Error> {
    if !dir.is_dir() {
        return Err(Error::NoProjectsDir { dir: dir.to_path_buf() });
    }
    let read = |d: &Path| std::fs::read_dir(d).map_err(|e| Error::ReadDir { dir: d.to_path_buf(), why: e.to_string() });
    let mut out = Vec::new();
    for project in read(dir)?.flatten() {
        let Ok(inner) = std::fs::read_dir(project.path()) else { continue };
        for f in inner.flatten() {
            let p = f.path();
            let recent = f.metadata().and_then(|m| m.modified()).map(|t| t >= since).unwrap_or(false);
            if recent && p.extension().is_some_and(|e| e == "jsonl") {
                out.push(p);
            }
        }
    }
    if out.is_empty() {
        let holds_jsonl = std::fs::read_dir(dir)
            .map(|d| d.flatten().any(|f| f.path().extension().is_some_and(|e| e == "jsonl")))
            .unwrap_or(false);
        if holds_jsonl {
            return Err(Error::ProjectFolder { dir: dir.to_path_buf() });
        }
        return Err(Error::NoTranscripts { dir: dir.to_path_buf() });
    }
    Ok(out)
}

fn run(cmd: Command) -> Result<String, Error> {
    let now = SystemTime::now();
    let now_ms = now.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    match cmd {
        Command::Help => Ok(USAGE.to_string()),
        Command::Runs { scope, projects, json } => {
            let (since, until) = scope.window.bounds(now_ms);
            let files = transcripts(&projects, UNIX_EPOCH + Duration::from_millis(since.0.max(0) as u64))?;
            let (sessions, left_out) = in_agents(&files, &scope, now_ms);
            let readings = sessions
                .into_iter()
                .map(|s| {
                    let events = s
                        .events
                        .into_iter()
                        .filter(|e| match e {
                            transcript::Event::Input { at, .. }
                            | transcript::Event::ToolResult { at }
                            | transcript::Event::AgentTurn { at, .. } => *at >= since && until.is_none_or(|u| *at < u),
                        })
                        .collect();
                    session::read_session(events)
                })
                .collect();
            let r = report::runs_report(&scope, files.len(), left_out, readings);
            Ok(if json { r.json().to_string() } else { r.text() })
        }
        Command::Escapes { scope, projects, model, ollama, json } => {
            let (since, until) = scope.window.bounds(now_ms);
            let oldest = UNIX_EPOCH + Duration::from_millis(since.0.max(0) as u64);
            let files = transcripts(&projects, oldest)?;
            let (sessions, left_out) = in_agents(&files, &scope, now_ms);
            let sessions: Vec<_> = sessions.into_iter().map(|s| s.events).collect();
            let untagged = sessions
                .iter()
                .filter(|events| !events.iter().any(|e| matches!(e, transcript::Event::Input { tagged: true, .. })))
                .count();
            let turns: Vec<_> = sessions
                .into_iter()
                .flat_map(escapes::human_turns)
                .filter(|turn| turn.at >= since && until.is_none_or(|u| turn.at < u))
                .collect();
            if !turns.is_empty() {
                escapes::preflight(&ollama, &model)?;
            }
            let mut labelled = Vec::new();
            for turn in &turns {
                labelled.push((turn.at, escapes::label(&ollama, &model, turn)?));
            }
            let r = report::escapes_report(model, &scope, (since, until), left_out, untagged, labelled);
            Ok(if json { r.json().to_string() } else { r.text() })
        }
        Command::Andon { days, hub_db, json } => {
            let r = andon::read(&hub_db, days, now_ms)?;
            Ok(if json { r.json().to_string() } else { r.text() })
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args).and_then(run) {
        Ok(out) => println!("{out}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}
