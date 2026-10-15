# Ask box: which model (evaluation, 2026-10-12)

**Recommendation:** the default is **Claude Sonnet 5.5, low effort**. The ⚡ fast option is **GPT-6-Luna, low effort**, and it's off at start.

Luna clears the bar with prompt v2: 100 % of answers parse, and 93 % of expectations are met over 3 runs. But it isn't meaningfully faster than Sonnet (median 4.2 s vs 4.5 s), and about once in 20 asks it quietly drops points from a blob. Its real advantages are that it runs on the ChatGPT plan and is cheaper. Sonnet was perfect on all 32 tasks.

Settings: `studio = { figure_model, figure_effort, figure_fast_model, figure_fast_effort, figure_fast }` in prefs.lua.

## Setup

- **Tasks:** 32 in `tasks.txt`, written in the author's voice: 12 commutative diagrams, 7 blobs, 6 n-gons, 4 Cayley graphs and 3 tidy-ups.
- **How answers are checked:** against the *parsed* figure, not the exact text. Each task has a hand-written reference answer in `reference/` that passes its own checks.
- **"Preserved":** untouched statements, their order, and every comment survive.
- **Prompt:** `figure_ai::system_prompt()`, prompt version 2. v2 was tuned after seeing v1's misses on these tasks, so its numbers are somewhat optimistic.

## Results (prompt v2, the one shipped)

| config | asks | model calls | parse-clean | first-try clean | expectations met | checks passed | preserved | all good | tasks good in every run | median s | p90 s | failures |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| luna-low | 96 | 96 | 100% | 100% | 93% | 98% | 97% | 92% | 27/32 | 4.2 | 6.9 | unpreserved 1, wrong 5, wrong+unpreserved 2 |
| luna-low-priority | 32 | 32 | 100% | 100% | 88% | 96% | 97% | 88% | 28/32 | 3.9 | 6.5 | wrong 3, wrong+unpreserved 1 |
| sonnet-low | 32 | 32 | 100% | 100% | 100% | 100% | 100% | 100% | 32/32 | 4.5 | 6.4 | – |
| sol-low | 32 | 32 | 100% | 100% | 100% | 100% | 100% | 100% | 32/32 | 9.9 | 13.0 | – |

All good, by task kind:

| config | cd | blob | ngon | cayley | tidy |
|---|---:|---:|---:|---:|---:|
| luna-low | 86% | 86% | 100% | 100% | 100% |
| luna-low-priority | 75% | 86% | 100% | 100% | 100% |
| sonnet-low | 100% | 100% | 100% | 100% | 100% |
| sol-low | 100% | 100% | 100% | 100% | 100% |

Tasks not good in every run (good/runs):

- **luna-low**: bend-iso 1/3, equals-grid 1/3, function-arrows 2/3, kernel-to-identity 1/3, triangle-to-square 2/3
- **luna-low-priority**: bend-iso 0/1, equals-grid 0/1, kernel-to-identity 0/1, triangle-to-square 0/1
- **sonnet-low**: none
- **sol-low**: none

**Luna's v2 misses**, in detail:
- **kernel-to-identity**, 2 of 3 runs: puts `e'` in a new second kernel blob inside H.
- **equals-grid**, 2 of 3: writes the inclusion as a plain `|i` in the grid, or draws it twice.
- **function-arrows**, 1 of 3: deletes elements from A and B.
- **triangle-to-square**, 1 of 3: puts the label on a solid arrow instead of the dashed diagonal.
- **bend-iso**, 2 of 3 (and the one priority run): bends A→B left and B→A right. A bend is relative to the arrow's direction, so both curve to the same side and still sit on top of each other.

**Prompt v1 → v2.** With v1, 3 runs each, the share of expectations met was luna-low 83 %, luna-medium 84 %, haiku-low 90 % and sonnet-low 96%. Every answer parsed, and none needed the retry. v2 spells out the details Luna kept missing:
- every mention of a name is the same node;
- `turn=k` (not `rotation`) shows the polygon after a rotation;
- a grid can't show a hook or a long label;
- one map is one statement;
- groups are written `ZmxZn`, with explicit generators.

The full v1 tables are in `results/v1/table.md`.

**Priority tier (A/B, 10 tasks, run one after another):** median 4.03 s on the default tier vs 3.93 s on priority, and means 4.31 s vs 4.30 s. It buys nothing for answers this short, so it's off; `NTEX_STUDIO_CODEX_TIER=priority` turns it on.

**Cost and time:** about 602 model calls, all on her subscriptions (596 asks, 4 flag probes, 2 real test calls). About 13 min wall time and 48 model-minutes.

**Not run** (to stay near 600 calls): Haiku at medium effort, Sonnet at medium, and Luna at medium on v2. Sonnet, Sol and Luna-priority got one v2 run each.

## Scorer fixes (2026-10-12, after the runs)

- **Bend direction now counts.** The bare `bend` check passed an arrow bent to either side, so on `bend-iso` it credited `A -f-> B bend-left` with `B -g-> A bend-right`, two arrows curving to the same side, i.e. still overlapping. New `apart A B` check: the arrows between A and B must curve to different sides (bends are read relative to each arrow's direction). New `bend=left|right` for tasks where the side itself matters.
- **`image-blob` respects Lagrange.** H was `{e', h, k}`, a group of order 3 holding a 2-element image, which is impossible. H is now `{e', h, k, hk}` (the Klein four-group, with φ: Z4 → H, g ↦ h, image `{e', h}`, kernel `{e, g^2}` as in `kernel-blob`). Same ask; the reference and checks match.
- **Luna's true score is a little lower than first reported.** The lenient bend check credited 3 of her v2 answers and 4 of her v1 answers. All stored answers were re-scored offline with `rescore` (no model calls). The tables above are the corrected ones. Before → after: luna-low 95 → 93 % expectations met, 94 → 92 % all good, 28 → 27/32 tasks good in every run; luna-low-priority 91 → 88 %, 91 → 88 %, 29 → 28/32. v1: luna-low 85 → 83 %, luna-medium 86 → 84 %. Sonnet, Sol and Haiku don't change, so the recommendation stands.
- The stored `image-blob` answers were given to the old task, so they keep their old verdicts (all passed). `rescore` now stamps each answer with a fingerprint of its task's ask and source, and never re-judges an answer to an older version of a task. Fresh numbers on the new `image-blob` need a new run.

## Findings outside the model choice

- **Her email reaches Anthropic.** `claude -p` adds the account's email to the model's context even with `--safe-mode` and our own system prompt. Only `--bare` drops it, and that needs an API key, which isn't set. Our own prompts contain no personal data. The Codex path sends none.
- **`cayley Z2 x Z4` without `<generators>`** was read as just `Z2`. The prompt asks for `Z2xZ4` with explicit generators, and the parser is being fixed.
