# lightsout

How long do your coding agents run between human touches? `lightsout runs` reads Claude Code transcripts (`~/.claude/projects/*/*.jsonl`) and prints, for every human turn, the span until the agent's last turn before the next human turn. Input from other agents, hooks, channels and background jobs doesn't end a span: that is the unattended work.

## Install

You need Rust (install it with [rustup](https://rustup.rs)) and Claude Code, used at least once so it has written transcripts.

```
cd lightsout            # the folder holding this README
cargo install --path .  # puts `lightsout` on your PATH (or: cargo build --release, then ./target/release/lightsout)
```

## Use

```
lightsout runs --days 7      # needs only your transcripts. Add --json for machine output
lightsout escapes --days 1   # needs Ollama and a model (below)
lightsout andon --days 14    # needs the command hub that runs on the author's machine; elsewhere it says so and stops
```

Transcripts are read from `~/.claude/projects` (or `$CLAUDE_CONFIG_DIR/projects`). Point `--projects DIR` elsewhere if yours live somewhere else; DIR is the folder that holds one folder per project.

`runs` prints something like this (a real week of one person's use):

```
lightsout runs: last 7 days, 1230 transcript files
sessions: 122 with origin tags, 1108 without (written before Claude Code tagged origins; not measured), 43 never touched by a human
span from each human touch to the agent's last turn before the next (waits included): n=1680  median 0.5 min  p90 6.2  p99 50.0  mean 3.7  total 102.8 h
span of agent turns before a session's first human touch (whole session if never touched): n=48  median 7.7 min  p90 172.6  p99 452.0  mean 49.1  total 39.3 h
inputs by sender:
  human: 1680
  peer: 854
  ...
```

Read the median and the p90: half of the touches were followed by half a minute of agent work or less, one in ten by six minutes or more.

`escapes` labels each of your turns with a local model, so nothing leaves your machine and it costs nothing. Install [Ollama](https://ollama.com), run `ollama serve`, then `ollama pull qwen3.5:9b-q8_0` (about 10 GB; a smaller model works with `--model`, but check it first). Its prompt is in `src/escapes.rs`. The sample it mentions is a set of human turns, 11 of them real escapes, that the author labelled by hand to check the model.

Watch two numbers together: minutes per touch (`runs`) should rise, and the share of your turns that catch a defect (`escapes`) should fall. `escapes` reads as a floor. Check it against your own blind labels on a sample first.

Read-only. Only sessions whose transcripts carry `origin` tags (recent Claude Code versions) are measured; older ones are counted and left out.

Why: a factory runs lights-out for as long as the time between abnormalities it can't handle itself. The research behind this tool is in the MBSI library, `wiki/how-to-run-a-lights-out-software-factory.md`.
