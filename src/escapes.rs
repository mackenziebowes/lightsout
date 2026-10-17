//! Escapes: how often a human turn says an agent's work or claim was wrong, labelled by a local model through Ollama. Promise: the rate it prints is a floor, read off what the model marked; it never guesses a turn it couldn't label.

use crate::error::Error;
use crate::transcript::{Event, InputFrom, Instant};
use serde_json::{Value, json};
use std::time::Duration;

/// The instrument. Changing a word changes the measure, so a change gets checked against blind labels again.
pub const PROMPT: &str = r#"You label one message a person sent to an AI agent.
Question: does this message say that the agent's own earlier work or claim was WRONG?
Label "escape" only when the person names a fault in something the agent already delivered, built, wrote, claimed or did: it is wrong, broken, contradictory, missing something that was asked, or done in a way the person objects to. Examples of escape: "2 is wrong", "your flow UI failed in this html", "assumptions 2 and 3 seem to argue", "why are we committing mesh files", "you didn't tell me ticket velocity took an hour", "what a mess, did you learn anything".
Label "other" for everything else, including: new requests or next steps; answers to the agent's question; extra information or context ("it goes up by about $600", "I saw the bug when X"); preferences and ideas; questions; approvals and thanks; greetings; a bug in some other program the agent did not make; disagreeing with a proposal the agent has not built yet.
If unsure, answer "other".
Answer with JSON only: {"label":"escape"} or {"label":"other"}."#;

pub struct HumanTurn {
    pub at: Instant,
    pub said: String,
    pub agent_before: String,
}

pub enum Label {
    Escape,
    Other,
}

/// The human turns of one session, each with the agent's last text before it. Slash commands are left out.
pub fn human_turns(mut events: Vec<Event>) -> Vec<HumanTurn> {
    events.sort_by(|a, b| at(a).partial_cmp(&at(b)).unwrap_or(std::cmp::Ordering::Equal));
    let mut last_agent = String::new();
    let mut out = Vec::new();
    for e in events {
        match e {
            Event::AgentTurn { text, .. } if !text.trim().is_empty() => last_agent = text,
            Event::Input { at, from: InputFrom::Human, tagged: true, text }
                if !text.trim().is_empty() && !text.starts_with("<command-") && !text.starts_with("<local-command") =>
            {
                out.push(HumanTurn { at, said: text, agent_before: last_agent.clone() })
            }
            _ => {}
        }
    }
    out
}

fn at(e: &Event) -> Instant {
    match e {
        Event::Input { at, .. } | Event::ToolResult { at } | Event::AgentTurn { at, .. } => *at,
    }
}

fn tail(s: &str, chars: usize) -> String {
    let n = s.chars().count();
    s.chars().skip(n.saturating_sub(chars)).collect()
}

fn head(s: &str, chars: usize) -> String {
    s.chars().take(chars).collect()
}

/// The Ollama edge: one chat call, temperature 0, answer held to the label schema.
pub fn label(ollama: &str, model: &str, turn: &HumanTurn) -> Result<Label, Error> {
    let body = json!({
        "model": model, "stream": false, "think": false,
        "format": {"type": "object", "properties": {"label": {"enum": ["escape", "other"]}}, "required": ["label"]},
        "options": {"temperature": 0, "num_ctx": 4096},
        "messages": [
            {"role": "system", "content": PROMPT},
            {"role": "user", "content": format!("The agent's last message (end):\n{}\n\nThe person's message:\n{}", tail(&turn.agent_before, 600), head(&turn.said, 900))}
        ]
    });
    let fail = |why: String| Error::Ollama { url: ollama.to_string(), model: model.to_string(), why };
    let mut resp = ureq::post(&format!("{ollama}/api/chat"))
        .config()
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .send_json(body)
        .map_err(|e| fail(e.to_string()))?;
    let text = resp.body_mut().read_to_string().map_err(|e| fail(e.to_string()))?;
    let reply: Value = serde_json::from_str(&text).map_err(|e| fail(e.to_string()))?;
    let content = reply.pointer("/message/content").and_then(Value::as_str).unwrap_or("");
    let picked: Value = serde_json::from_str(content).unwrap_or(Value::Null);
    match picked.get("label").and_then(Value::as_str) {
        Some("escape") => Ok(Label::Escape),
        Some("other") => Ok(Label::Other),
        _ => Err(fail(format!("the model's answer was not a label: {}", head(content, 120)))),
    }
}
