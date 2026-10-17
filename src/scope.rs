//! What to measure: a window of time and a set of agents. Promise: a session is in scope when it has a turn in the window and, if agents are named, carries one of those names.

use crate::transcript::Instant;

pub enum Window {
    Days(u32),
    Span { since: Instant, until: Option<Instant> },
}

pub enum Agents {
    Any,
    /// Names as the transcript's `agent-name` records spell them; matched ignoring case.
    Named(Vec<String>),
}

pub struct Scope {
    pub window: Window,
    pub agents: Agents,
}

impl Window {
    /// The window as fixed instants, counting `Days` back from `now_ms`.
    pub fn bounds(&self, now_ms: i64) -> (Instant, Option<Instant>) {
        match self {
            Window::Days(d) => (Instant(now_ms - i64::from(*d) * 86_400_000), None),
            Window::Span { since, until } => (*since, *until),
        }
    }

    pub fn words(&self) -> String {
        match self {
            Window::Days(d) => format!("last {d} days"),
            Window::Span { since, until: Some(u) } => format!("{} to {}", since.iso(), u.iso()),
            Window::Span { since, until: None } => format!("since {}", since.iso()),
        }
    }
}

impl Agents {
    /// Splits `Keel,Harbor` into names. Empty names are refused by the caller.
    pub fn parse(list: &str) -> Option<Agents> {
        let names: Vec<String> = list.split(',').map(|n| n.trim().to_string()).collect();
        if names.iter().any(|n| n.is_empty()) {
            return None;
        }
        Some(Agents::Named(names))
    }

    pub fn admits(&self, session_names: &[String]) -> bool {
        match self {
            Agents::Any => true,
            Agents::Named(wanted) => session_names.iter().any(|s| wanted.iter().any(|w| w.eq_ignore_ascii_case(s))),
        }
    }

    pub fn words(&self) -> Option<String> {
        match self {
            Agents::Any => None,
            Agents::Named(names) => Some(names.join(", ")),
        }
    }
}
