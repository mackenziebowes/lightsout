//! One session as a state machine: a run starts at each human touch and lasts until the agent's last turn before the next one. Promise: every agent turn after a human touch is counted in exactly one run, and input from anyone else (peers, hooks, channels) never ends a run.

use crate::transcript::{Event, InputFrom, Instant};

pub enum State {
    /// No human touch yet; agent turns here were fed by someone else.
    Unattended { first: Option<Instant>, last: Option<Instant> },
    Running { started: Instant, last_agent: Instant },
}

pub struct Run {
    pub started: Instant,
    pub last_agent: Instant,
}

impl Run {
    pub fn minutes(&self) -> f64 {
        self.last_agent.minutes_since(self.started)
    }
}

/// What a transition hands back besides the next state.
pub enum Effect {
    None,
    RunEnded(Run),
    /// The first human touch closed a stretch the agent worked with no human.
    UnattendedEnded { minutes: f64 },
}

pub fn step(state: State, event: &Event) -> (State, Effect) {
    match (state, event) {
        (State::Unattended { first, last }, Event::Input { at, from: InputFrom::Human, .. }) => {
            let minutes = match (first, last) {
                (Some(f), Some(l)) => l.minutes_since(f),
                _ => 0.0,
            };
            (State::Running { started: *at, last_agent: *at }, Effect::UnattendedEnded { minutes })
        }
        (State::Running { started, last_agent }, Event::Input { at, from: InputFrom::Human, .. }) => (
            State::Running { started: *at, last_agent: *at },
            Effect::RunEnded(Run { started, last_agent }),
        ),
        (State::Running { started, .. }, Event::AgentTurn { at, .. }) => {
            (State::Running { started, last_agent: *at }, Effect::None)
        }
        (State::Unattended { first, .. }, Event::AgentTurn { at, .. }) => {
            (State::Unattended { first: first.or(Some(*at)), last: Some(*at) }, Effect::None)
        }
        (state, Event::Input { .. } | Event::ToolResult { .. }) => (state, Effect::None),
    }
}

/// What one session file adds up to.
pub enum SessionReading {
    /// The transcript predates `origin` tags, so human turns can't be told from others.
    Untagged,
    Tagged {
        runs: Vec<Run>,
        /// Agent minutes before the first human touch (a session fed only by peers or hooks has all its work here).
        unattended_minutes: f64,
        inputs: Vec<InputFrom>,
    },
}

pub fn read_session(mut events: Vec<Event>) -> SessionReading {
    let tagged = events.iter().any(|e| matches!(e, Event::Input { tagged: true, .. }));
    if !tagged {
        return SessionReading::Untagged;
    }
    events.sort_by(|a, b| at(a).partial_cmp(&at(b)).unwrap_or(std::cmp::Ordering::Equal));
    let mut state = State::Unattended { first: None, last: None };
    let mut runs = Vec::new();
    let mut inputs = Vec::new();
    let mut unattended_minutes = 0.0;
    for event in &events {
        if let Event::Input { from, .. } = event {
            inputs.push(from.clone());
        }
        let (next, effect) = step(state, event);
        state = next;
        match effect {
            Effect::None => {}
            Effect::RunEnded(run) => runs.push(run),
            Effect::UnattendedEnded { minutes } => unattended_minutes = minutes,
        }
    }
    match state {
        State::Running { started, last_agent } => runs.push(Run { started, last_agent }),
        State::Unattended { first: Some(f), last: Some(l) } => unattended_minutes = l.minutes_since(f),
        State::Unattended { .. } => {}
    }
    SessionReading::Tagged { runs, unattended_minutes, inputs }
}

fn at(e: &Event) -> Instant {
    match e {
        Event::Input { at, .. } | Event::ToolResult { at } | Event::AgentTurn { at, .. } => *at,
    }
}
