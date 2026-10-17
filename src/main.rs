//! lightsout: how long coding agents run per human touch. `runs` reads Claude Code transcripts (any machine); `andon` reads this machine's command hub. Promise: read-only on everything it reads, and every gauge it cannot measure says so.

mod andon;
mod error;
mod report;
mod session;
mod transcript;

use error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

enum Command {
    Runs { days: u32, projects: PathBuf, json: bool },
    Andon { days: u32, hub_db: PathBuf, json: bool },
    Help,
}

const USAGE: &str = "lightsout: how long coding agents run per human touch

  lightsout runs  [--days 7] [--projects ~/.claude/projects] [--json]
      agent minutes after each human turn, from Claude Code transcripts
  lightsout andon [--days 14] [--hub-db ~/.local/share/command-hub/hub.db] [--json]
      the command hub's asks, answer times, withdrawals and findings";

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

fn parse_args(args: &[String]) -> Result<Command, Error> {
    let Some(cmd) = args.first() else { return Ok(Command::Help) };
    let mut days: Option<u32> = None;
    let mut path: Option<PathBuf> = None;
    let mut json = false;
    let path_flag = match cmd.as_str() {
        "runs" => "--projects",
        "andon" => "--hub-db",
        "help" | "--help" | "-h" => return Ok(Command::Help),
        other => return Err(Error::UnknownCommand { given: other.to_string() }),
    };
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json = true,
            "--days" => {
                let v = it.next().ok_or(Error::MissingValue { flag: "--days" })?;
                days = Some(v.parse::<u32>().ok().filter(|d| *d > 0).ok_or(Error::BadDays { given: v.clone() })?);
            }
            f if f == path_flag => {
                let v = it.next().ok_or(Error::MissingValue { flag: path_flag })?;
                path = Some(PathBuf::from(v));
            }
            other => return Err(Error::UnknownFlag { given: other.to_string() }),
        }
    }
    Ok(match cmd.as_str() {
        "runs" => Command::Runs {
            days: days.unwrap_or(7),
            projects: path.unwrap_or_else(|| home().join(".claude/projects")),
            json,
        },
        _ => Command::Andon {
            days: days.unwrap_or(14),
            hub_db: path.unwrap_or_else(|| home().join(".local/share/command-hub/hub.db")),
            json,
        },
    })
}

fn transcripts(dir: &Path, since: SystemTime) -> Result<Vec<PathBuf>, Error> {
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
        return Err(Error::NoTranscripts { dir: dir.to_path_buf() });
    }
    Ok(out)
}

fn run(cmd: Command) -> Result<String, Error> {
    let now = SystemTime::now();
    let now_ms = now.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    match cmd {
        Command::Help => Ok(USAGE.to_string()),
        Command::Runs { days, projects, json } => {
            let since = now - Duration::from_secs(u64::from(days) * 86_400);
            let cutoff = transcript::Instant(now_ms - i64::from(days) * 86_400_000);
            let files = transcripts(&projects, since)?;
            let readings = files
                .iter()
                .map(|f| {
                    let text = std::fs::read_to_string(f).unwrap_or_default();
                    let events = text
                        .lines()
                        .filter_map(|l| transcript::parse_line(l).ok())
                        .filter(|e| match e {
                            transcript::Event::Input { at, .. }
                            | transcript::Event::ToolResult { at }
                            | transcript::Event::AgentTurn { at } => *at >= cutoff,
                        })
                        .collect();
                    session::read_session(events)
                })
                .collect();
            let r = report::runs_report(days, files.len(), readings);
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
