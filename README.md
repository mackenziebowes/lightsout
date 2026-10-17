# lightsout

How long do your coding agents run between human touches? `lightsout runs` reads Claude Code transcripts (`~/.claude/projects/*/*.jsonl`) and prints, for every human turn, the span until the agent's last turn before the next human turn. Input from other agents, hooks, channels and background jobs doesn't end a span: that is the unattended work.

```
cargo build --release
./target/release/lightsout runs --days 7          # or --json
./target/release/lightsout andon --days 14        # the command hub's asks and findings (this machine only)
```

Read-only. Only sessions whose transcripts carry `origin` tags (recent Claude Code versions) are measured; older ones are counted and left out.

Why: a factory runs lights-out for as long as the time between abnormalities it can't handle itself. The research behind this tool is in the MBSI library, `wiki/how-to-run-a-lights-out-software-factory.md`.
