# Ask-box evaluation (FIG-AI)

Does a model rewrite a `.fig` file well from one instruction? Every task goes
through the real `io::figure_ai::AskJob` (same system prompt, flags, retry
and 60 s timeout as the figure window), and the answer is checked against
the **parsed** figure, never by exact text.

- `tasks.txt` holds the tasks, and is hand-editable. Its header lists the checks.
- `src/main.rs` is the harness.
- `results/<config>.jsonl` gets one line per ask. Delete a file to re-run that config.
- `results/table.md` holds the generated table.
- `REPORT.md` holds the numbers and the recommendation.

```sh
cd ~/.config/texnvim/studio/figures/eval
cargo run --release -- check                 # the tasks themselves; no model calls
cargo run --release -- configs
cargo run --release -- run luna-low 3        # 3 runs of every task; resumes, never repeats
cargo run --release -- run sonnet-low 1 --tasks pullback,ses --jobs 2
cargo run --release -- report
cargo run --release -- try pullback my-answer.fig   # test a check by hand
```

`run` costs model calls on the author's subscriptions: one call per ask, two
when the answer needed the retry. `check`, `report` and `try` cost nothing.
This is its own little Cargo workspace, so the studio's `cargo test` never
builds or runs it.

What a task's **touch** line means: the statements the instruction may change.
Everything else in the starting source must come back semantically equal (as
the parser reads it; spans ignored) and in the same order. Comments are always
kept.
