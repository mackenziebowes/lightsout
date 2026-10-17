//! The edge: turns one Claude Code transcript line (JSONL) into an `Event`, or says why it is skipped. Nothing past this module looks at JSON.

use serde_json::Value;

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct Instant(pub i64);

impl Instant {
    /// Parses `2026-10-16T21:15:03.433Z` (UTC, as Claude Code writes it).
    pub fn parse(s: &str) -> Option<Instant> {
        let b = s.as_bytes();
        if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || !s.ends_with('Z') {
            return None;
        }
        let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
        let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
        let (h, mi, sec) = (num(11..13)?, num(14..16)?, num(17..19)?);
        let ms = match s.get(19..s.len() - 1) {
            Some("") => 0,
            Some(frac) if frac.starts_with('.') => {
                let digits = &frac[1..];
                let padded = format!("{digits:0<3}");
                padded.get(0..3)?.parse::<i64>().ok()?
            }
            _ => return None,
        };
        // days_from_civil (Howard Hinnant)
        let y2 = if mo <= 2 { y - 1 } else { y };
        let era = y2.div_euclid(400);
        let yoe = y2 - era * 400;
        let mp = (mo + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        Some(Instant(((days * 24 + h) * 60 + mi) * 60_000 + sec * 1000 + ms))
    }

    /// `2026-10-16T21:15:03Z`, the inverse of `parse` to the second.
    pub fn iso(self) -> String {
        let secs = self.0.div_euclid(1000);
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        // civil_from_days (Howard Hinnant)
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = yoe + era * 400 + i64::from(m <= 2);
        format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
    }

    pub fn minutes_since(self, earlier: Instant) -> f64 {
        (self.0 - earlier.0) as f64 / 60_000.0
    }
}

/// Who sent a user turn. Read from `origin.kind`; a turn with no origin is named by what the harness wrote.
#[derive(Clone, PartialEq)]
pub enum InputFrom {
    Human,
    Peer,
    Channel,
    Plugin,
    TaskNotification,
    /// Text the harness injects with no origin: stop-hook feedback, idle notices, skill bodies, reminders.
    Harness,
    /// An origin kind this version doesn't know yet.
    OtherOrigin { kind: String },
}

pub enum Event {
    Input { at: Instant, from: InputFrom, tagged: bool, text: String },
    ToolResult { at: Instant },
    AgentTurn { at: Instant, text: String },
}

pub enum Skip {
    NotJson,
    Sidechain,
    NotATurn,
    NoTimestamp,
}

pub fn parse_line(line: &str) -> Result<Event, Skip> {
    let v: Value = serde_json::from_str(line).map_err(|_| Skip::NotJson)?;
    if v.get("isSidechain").and_then(Value::as_bool) == Some(true) {
        return Err(Skip::Sidechain);
    }
    let kind = v.get("type").and_then(Value::as_str).ok_or(Skip::NotATurn)?;
    if kind != "user" && kind != "assistant" {
        return Err(Skip::NotATurn);
    }
    let at = v
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(Instant::parse)
        .ok_or(Skip::NoTimestamp)?;
    let content = v.pointer("/message/content");
    if kind == "assistant" {
        return Ok(Event::AgentTurn { at, text: all_text(content) });
    }
    let is_tool_result = content
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|first| first.get("type"))
        .and_then(Value::as_str)
        == Some("tool_result");
    if is_tool_result {
        return Ok(Event::ToolResult { at });
    }
    match v.pointer("/origin/kind").and_then(Value::as_str) {
        Some(origin) => Ok(Event::Input { at, from: from_origin(origin), tagged: true, text: all_text(content) }),
        None => {
            let text = all_text(content);
            let from = if text.starts_with("[Request interrupted by user") { InputFrom::Human } else { InputFrom::Harness };
            Ok(Event::Input { at, from, tagged: false, text })
        }
    }
}

fn from_origin(kind: &str) -> InputFrom {
    match kind {
        "human" => InputFrom::Human,
        "peer" => InputFrom::Peer,
        "channel" => InputFrom::Channel,
        "plugin" => InputFrom::Plugin,
        "task-notification" => InputFrom::TaskNotification,
        other => InputFrom::OtherOrigin { kind: other.to_string() },
    }
}

fn all_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter(|i| i.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|i| i.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// The name Claude Code gives a session in an `agent-name` record, if this line is one.
pub fn agent_name(line: &str) -> Option<String> {
    if !line.contains("\"agent-name\"") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(Value::as_str) != Some("agent-name") {
        return None;
    }
    v.get("agentName").and_then(Value::as_str).map(str::to_string)
}

/// How Claude Code was launched for this line (`cli` for a person at the terminal, `sdk-cli` for a headless run).
pub fn entrypoint(line: &str) -> Option<String> {
    if !line.contains("\"entrypoint\"") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    v.get("entrypoint").and_then(Value::as_str).map(str::to_string)
}
