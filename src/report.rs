//! Turns readings into the numbers people see, and holds every word of the printed report. Promise: a gauge with nothing to measure prints as unmeasured, never as 0.

use crate::escapes::Label;
use crate::session::SessionReading;
use crate::transcript::{InputFrom, Instant};
use serde_json::{Value, json};

pub enum Spread {
    Measured { n: usize, p50: f64, p90: f64, p99: f64, mean: f64, total: f64 },
    Unmeasured,
}

impl Spread {
    pub fn of(mut xs: Vec<f64>) -> Spread {
        if xs.is_empty() {
            return Spread::Unmeasured;
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let q = |p: f64| xs[((p * xs.len() as f64) as usize).min(xs.len() - 1)];
        let total: f64 = xs.iter().sum();
        Spread::Measured { n: xs.len(), p50: q(0.5), p90: q(0.9), p99: q(0.99), mean: total / xs.len() as f64, total }
    }

    pub fn json(&self) -> Value {
        match self {
            Spread::Measured { n, p50, p90, p99, mean, total } => {
                json!({"kind": "measured", "n": n, "p50": p50, "p90": p90, "p99": p99, "mean": mean, "total": total})
            }
            Spread::Unmeasured => json!({"kind": "unmeasured"}),
        }
    }

    fn line(&self, label: &str, unit: &str) -> String {
        match self {
            Spread::Measured { n, p50, p90, p99, mean, total } => format!(
                "{label}: n={n}  median {p50:.1} {unit}  p90 {p90:.1}  p99 {p99:.1}  mean {mean:.1}  total {:.1} h",
                total / 60.0
            ),
            Spread::Unmeasured => format!("{label}: unmeasured (nothing in the window)"),
        }
    }
}

fn sessions(n: usize) -> String {
    if n == 1 { "1 session".into() } else { format!("{n} sessions") }
}

pub struct RunsReport {
    pub days: u32,
    pub files: usize,
    pub untagged_sessions: usize,
    pub tagged_sessions: usize,
    pub sessions_never_touched: usize,
    pub runs: Spread,
    pub unattended_before_first_touch: Spread,
    pub inputs: Vec<(String, usize)>,
}

pub fn runs_report(days: u32, files: usize, readings: Vec<SessionReading>) -> RunsReport {
    let mut untagged = 0;
    let mut tagged = 0;
    let mut never_touched = 0;
    let mut run_minutes = Vec::new();
    let mut unattended = Vec::new();
    let mut counts: Vec<(String, usize)> = Vec::new();
    for r in readings {
        match r {
            SessionReading::Untagged => untagged += 1,
            SessionReading::Tagged { runs, unattended_minutes, inputs } => {
                tagged += 1;
                if runs.is_empty() {
                    never_touched += 1;
                }
                if unattended_minutes > 0.0 {
                    unattended.push(unattended_minutes);
                }
                run_minutes.extend(runs.iter().map(|r| r.minutes()));
                for from in inputs {
                    let name = input_name(&from);
                    match counts.iter_mut().find(|(n, _)| *n == name) {
                        Some((_, c)) => *c += 1,
                        None => counts.push((name, 1)),
                    }
                }
            }
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    RunsReport {
        days,
        files,
        untagged_sessions: untagged,
        tagged_sessions: tagged,
        sessions_never_touched: never_touched,
        runs: Spread::of(run_minutes),
        unattended_before_first_touch: Spread::of(unattended),
        inputs: counts,
    }
}

fn input_name(from: &InputFrom) -> String {
    match from {
        InputFrom::Human => "human".into(),
        InputFrom::Peer => "peer".into(),
        InputFrom::Channel => "channel".into(),
        InputFrom::Plugin => "plugin".into(),
        InputFrom::TaskNotification => "task-notification".into(),
        InputFrom::Harness => "harness (untagged)".into(),
        InputFrom::OtherOrigin { kind } => format!("other: {kind}"),
    }
}

impl RunsReport {
    pub fn json(&self) -> Value {
        json!({
            "days": self.days,
            "files": self.files,
            "sessions": {"tagged": self.tagged_sessions, "untagged": self.untagged_sessions, "neverTouched": self.sessions_never_touched},
            "agentMinutesPerHumanTouch": self.runs.json(),
            "unattendedMinutesBeforeFirstTouch": self.unattended_before_first_touch.json(),
            "inputs": self.inputs.iter().map(|(k, n)| json!({"from": k, "n": n})).collect::<Vec<_>>(),
        })
    }

    pub fn text(&self) -> String {
        let mut out = vec![
            format!("lightsout runs: last {} days, {} transcript files", self.days, self.files),
            format!(
                "sessions: {} with origin tags, {} without (written before Claude Code tagged origins; not measured), {} never touched by a human",
                self.tagged_sessions, self.untagged_sessions, self.sessions_never_touched
            ),
            self.runs.line("span from each human touch to the agent's last turn before the next (waits included)", "min"),
            self.unattended_before_first_touch.line("span of agent turns before a session's first human touch (whole session if never touched)", "min"),
            "inputs by sender:".into(),
        ];
        out.extend(self.inputs.iter().map(|(k, n)| format!("  {k}: {n}")));
        if self.tagged_sessions == 0 {
            out.push(format!(
                "nothing measured: every session in the window ({}) predates origin tags, so a human turn can't be told from a hook or another agent. Update Claude Code, use it, and run again",
                sessions(self.untagged_sessions)
            ));
        }
        out.join("\n")
    }
}

/// Of tickets done in the window that had a recorded review run, how many had no bug finding.
pub enum FirstPassYield {
    Measured { reviewed: i64, clean: i64 },
    Unmeasured,
}

pub struct AndonReport {
    pub first_pass_yield: FirstPassYield,
    pub days: u32,
    pub asks_by_shape: Vec<(String, i64)>,
    pub answers_by_shape_and_action: Vec<(String, String, i64)>,
    pub answer_minutes: Spread,
    pub withdrawn: i64,
    pub open: i64,
    pub findings_by_finder: Vec<(String, i64)>,
}

impl AndonReport {
    pub fn json(&self) -> Value {
        json!({
            "days": self.days,
            "asksByShape": self.asks_by_shape.iter().map(|(s, n)| json!({"shape": s, "n": n})).collect::<Vec<_>>(),
            "answers": self.answers_by_shape_and_action.iter().map(|(s, a, n)| json!({"shape": s, "action": a, "n": n})).collect::<Vec<_>>(),
            "answerMinutes": self.answer_minutes.json(),
            "withdrawn": self.withdrawn,
            "open": self.open,
            "firstPassYield": match &self.first_pass_yield {
                FirstPassYield::Measured { reviewed, clean } => json!({"kind": "measured", "reviewed": reviewed, "clean": clean}),
                FirstPassYield::Unmeasured => json!({"kind": "unmeasured"}),
            },
            "findingsByFinder": self.findings_by_finder.iter().map(|(f, n)| json!({"foundBy": f, "n": n})).collect::<Vec<_>>(),
        })
    }

    pub fn text(&self) -> String {
        let total: i64 = self.asks_by_shape.iter().map(|(_, n)| n).sum();
        let mut out = vec![format!("lightsout andon: hub asks opened in the last {} days: {total}", self.days)];
        out.extend(self.asks_by_shape.iter().map(|(s, n)| format!("  {s}: {n}")));
        out.push("answers by shape and action:".into());
        out.extend(self.answers_by_shape_and_action.iter().map(|(s, a, n)| format!("  {s} {a}: {n}")));
        out.push(self.answer_minutes.line("minutes from ask to answer", "min"));
        out.push(format!("withdrawn by the asker: {}   still open: {}", self.withdrawn, self.open));
        out.push(match &self.first_pass_yield {
            FirstPassYield::Measured { reviewed, clean } => format!(
                "first-pass yield: {clean} of {reviewed} done tickets with a review run had no bug finding ({:.0}%)",
                100.0 * *clean as f64 / *reviewed as f64
            ),
            FirstPassYield::Unmeasured => "first-pass yield: unmeasured (no done ticket in the window has a review run)".into(),
        });
        out.push("findings by who found them:".into());
        if self.findings_by_finder.is_empty() {
            out.push("  none in the window".into());
        }
        out.extend(self.findings_by_finder.iter().map(|(f, n)| format!("  {f}: {n}")));
        if !self.findings_by_finder.iter().any(|(f, _)| f == "mackenzie") {
            out.push("  (no finding is recorded as found by mackenzie: escapes past review are unmeasured, not zero)".into());
        }
        out.join("\n")
    }
}

pub struct EscapesReport {
    pub model: String,
    pub since: Instant,
    pub until: Option<Instant>,
    pub turns: usize,
    pub escapes: usize,
    pub untagged_sessions: usize,
}

pub fn escapes_report(model: String, since: Instant, until: Option<Instant>, untagged_sessions: usize, labelled: Vec<(Instant, Label)>) -> EscapesReport {
    let escapes = labelled.iter().filter(|(_, l)| matches!(l, Label::Escape)).count();
    EscapesReport { model, since, until, turns: labelled.len(), escapes, untagged_sessions }
}

impl EscapesReport {
    pub fn json(&self) -> Value {
        json!({
            "model": self.model,
            "sinceMs": self.since.0,
            "untilMs": self.until.map(|u| u.0),
            "untaggedSessionsSkipped": self.untagged_sessions,
            "rate": if self.turns == 0 { json!({"kind": "unmeasured"}) } else { json!({"kind": "floor", "escapes": self.escapes, "turns": self.turns}) },
        })
    }

    pub fn text(&self) -> String {
        if self.turns == 0 {
            return format!(
                "lightsout escapes: unmeasured (no human turns in the window; skipped {} that predate origin tags, because without tags a human turn can't be told from a hook or another agent)",
                sessions(self.untagged_sessions)
            );
        }
        format!(
            "lightsout escapes: {} of {} human turns say an agent's work or claim was wrong ({:.1}%), labelled by {}. Skipped {} that predate origin tags.\nThis is a floor: check the model against your own blind labels on a sample before trusting the number (on the author's hand-labelled sample of 11 escapes, see the README: precision 6/6, recall 6/11).",
            self.escapes,
            self.turns,
            100.0 * self.escapes as f64 / self.turns as f64,
            self.model,
            sessions(self.untagged_sessions)
        )
    }
}
