# lightsout

How long do your coding agents run between human touches? `lightsout runs` reads Claude Code transcripts (`~/.claude/projects/*/*.jsonl`) and prints, for every human turn, the span until the agent's last turn before the next human turn. Input from other agents, hooks, channels and background jobs doesn't end a span: that is the unattended work.

```
cargo build --release
./target/release/lightsout runs --days 7          # or --json
./target/release/lightsout andon --days 14        # the command hub's asks and findings (this machine only)
./target/release/lightsout escapes --days 1       # share of your turns that say an agent got it wrong (local Ollama model)
```

Watch two numbers together: minutes per touch (`runs`) should rise, and the share of your turns that catch a defect (`escapes`) should fall. `escapes` labels each human turn with a local model (default `qwen3.5:9b-q8_0`), so it costs nothing, and it reads as a floor. Check it against your own blind labels on a sample first.

Read-only. Only sessions whose transcripts carry `origin` tags (recent Claude Code versions) are measured; older ones are counted and left out.

Why: a factory runs lights-out for as long as the time between abnormalities it can't handle itself. The research behind this tool is in the MBSI library, `wiki/how-to-run-a-lights-out-software-factory.md`.
